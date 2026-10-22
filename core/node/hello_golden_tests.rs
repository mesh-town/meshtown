//! Golden test of `broadcast_hello` (`node/event_loop.rs`): the text of the periodic `hello`, the counter
//! and the behavior with an empty network and a broadcast error. Its own `Membership` records the broadcast bodies.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::{BoxFuture, Membership, NetError, NetResult, SwarmEvent};
use bytes::Bytes;
use std::sync::Mutex;
use tokio::sync::broadcast;

struct Recorder {
    bodies: Mutex<Vec<String>>,
    fail: bool,
}

impl Recorder {
    fn new(fail: bool) -> Self {
        Self {
            bodies: Mutex::new(Vec::new()),
            fail,
        }
    }
    fn bodies(&self) -> Vec<String> {
        self.bodies.lock().unwrap().clone()
    }
}

impl Membership for Recorder {
    fn join(&self, _peers: &[PeerId]) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn broadcast(&self, msg: Bytes) -> BoxFuture<'_, NetResult<()>> {
        self.bodies
            .lock()
            .unwrap()
            .push(String::from_utf8(msg.to_vec()).unwrap());
        let fail = self.fail;
        Box::pin(async move {
            if fail {
                Err(NetError::Closed)
            } else {
                Ok(())
            }
        })
    }
    fn subscribe(&self) -> broadcast::Receiver<SwarmEvent> {
        broadcast::channel(1).1
    }
}

fn members_with_one() -> Members {
    let members = Members::default();
    members.add_verified_conn(pid(7));
    members
}

fn hello_text(n: u64) -> String {
    format!(
        "hello from {} #{n}",
        crate::node::output::short_peer_id(&pid(1))
    )
}

#[tokio::test]
async fn empty_network_sends_nothing_and_keeps_counter() {
    // given: no members
    let swarm = Recorder::new(false);
    // when
    let n = broadcast_hello(&swarm, pid(1), &Members::default(), 5).await;
    // then
    assert_eq!(n, 5);
    assert!(swarm.bodies().is_empty(), "sent: {:?}", swarm.bodies());
}

#[tokio::test]
async fn hello_text_is_ten_lowercase_hex_and_counter_grows() {
    // given: one member
    let (swarm, members) = (Recorder::new(false), members_with_one());
    // when: two ticks in a row
    let first = broadcast_hello(&swarm, pid(1), &members, 0).await;
    let second = broadcast_hello(&swarm, pid(1), &members, first).await;
    // then
    assert_eq!((first, second), (1, 2));
    assert_eq!(swarm.bodies(), vec![hello_text(1), hello_text(2)]);
    let hex = swarm.bodies()[0].split(' ').nth(2).unwrap().to_string();
    assert!(
        hex.len() == 10
            && hex
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "id: {hex}"
    );
}

#[tokio::test]
async fn send_error_does_not_stop_and_counter_still_grows() {
    // given: the broadcast fails
    let (swarm, members) = (Recorder::new(true), members_with_one());
    // when
    let n = broadcast_hello(&swarm, pid(1), &members, 3).await;
    // then
    assert_eq!(n, 4);
    assert_eq!(swarm.bodies(), vec![hello_text(4)]);
}
