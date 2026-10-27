//! The `Binding` state of a UDP entry: it shows while a busy local port is retried, turns into `Listening` or
//! `BindFailed`, and is kept (not re-created) by a repeated reconcile and released by a close.

use std::time::Duration;

use super::udp_tests::{entry, eventually, setup, udp_taken, Peer};
use crate::ports_sharing::open::OpenState;
use crate::ports_sharing::Proto;

/// Waits until the UDP entry on `local` has left `Binding` for `Listening`.
pub(super) async fn listens(fx: &super::udp_tests::Fixture, local: u16) {
    eventually("the state is listening", || {
        fx.state(Proto::Udp, local) == OpenState::Listening
    })
    .await;
}

/// A socket that holds a loopback UDP port, and the port.
fn held_udp_port() -> (std::net::UdpSocket, u16) {
    let holder = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let local = holder.local_addr().unwrap().port();
    (holder, local)
}

#[tokio::test]
async fn udp_entry_is_binding_while_retrying() {
    // given: another socket holds the local port
    let (_holder, local) = held_udp_port();
    let fx = setup(Peer::Silent, false).await;

    // when: the entry is opened
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);

    // then: it is binding at once, not listening
    assert_eq!(fx.state(Proto::Udp, local), OpenState::Binding);
}

#[tokio::test]
async fn binding_becomes_listening_when_port_frees() {
    // given: a busy port and an entry that is binding
    let (holder, local) = held_udp_port();
    let fx = setup(Peer::Silent, false).await;
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
    assert_eq!(fx.state(Proto::Udp, local), OpenState::Binding);

    // when: the port is freed within the retry window
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(holder);

    // then: the entry listens
    eventually("the state is listening", || {
        fx.state(Proto::Udp, local) == OpenState::Listening
    })
    .await;
    assert!(udp_taken(local), "nobody listens on the local port");
}

#[tokio::test]
async fn reconcile_while_binding_keeps_entry() {
    // given: an entry that is binding on a busy port
    let (holder, local) = held_udp_port();
    let fx = setup(Peer::Silent, false).await;
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);

    // when: the same list is applied again and the port is freed
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
    assert_eq!(fx.state(Proto::Udp, local), OpenState::Binding);
    drop(holder);

    // then: one entry, one listener: it listens and stays so (a duplicate would fail on the busy port)
    eventually("the state is listening", || {
        fx.state(Proto::Udp, local) == OpenState::Listening
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(fx.opener.bind_states().len(), 1);
    assert_eq!(fx.state(Proto::Udp, local), OpenState::Listening);
}

#[tokio::test]
async fn close_while_binding_releases() {
    // given: an entry that is binding on a busy port
    let (holder, local) = held_udp_port();
    let fx = setup(Peer::Silent, false).await;
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);

    // when: the entry is closed, then the port is freed
    fx.opener.reconcile_listeners(vec![]);
    drop(holder);

    // then: the aborted task never takes the port, and no failure is recorded
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert!(!udp_taken(local), "the aborted task took the port");
    assert!(fx.opener.bind_states().is_empty());
}
