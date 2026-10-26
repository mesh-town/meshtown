//! Tests of when the opening side forgets the last dial error of a TCP entry: right after the next successful
//! dial, not when that connection ends.

use std::time::Duration;

use crate::members::membership::Members;
use crate::ports_sharing::dial::PortReach;
use crate::ports_sharing::open::Opener;
use crate::ports_sharing::test_support::{linked, serving_on};
use crate::ports_sharing::{Opened, Proto};
use crate::transport::net::facade::test_support::pid;
use crate::transport::net::MemNet;
use tokio::net::{TcpListener, TcpStream};

const LIMIT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn reach_is_cleared_when_the_dial_succeeds_not_when_the_connection_ends() {
    // given: a TCP entry whose peer is not a member yet; one connection was refused
    let service = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = service.local_addr().unwrap().port();
    let net = MemNet::new();
    let (a, _b) = linked(&net, serving_on(Proto::Tcp, port));
    let members = Members::default();
    let opener = Opener::new(a, members.clone());
    let local = free_tcp_port();
    let entry = Opened {
        node: pid(2),
        port,
        proto: Proto::Tcp,
        local,
    };
    opener.reconcile_listeners(vec![entry]);
    let _refused = connect_when_listening(local).await;
    eventually("offline is recorded", || {
        last_error(&opener) == Some(PortReach::Offline)
    })
    .await;

    // when: the peer becomes a member and a new connection is made and kept open
    members.add_verified_conn(pid(2));
    let _kept = connect_when_listening(local).await;
    let (_held, _) = within_limit(service.accept()).await;

    // then: the error is already gone while the connection is still open
    eventually("the error is cleared", || last_error(&opener).is_none()).await;
}

fn last_error(opener: &Opener) -> Option<PortReach> {
    opener
        .bind_states()
        .into_iter()
        .next()
        .and_then(|(_, _, reach)| reach)
}

async fn connect_when_listening(local: u16) -> TcpStream {
    let start = tokio::time::Instant::now();
    loop {
        match TcpStream::connect(("127.0.0.1", local)).await {
            Ok(conn) => return conn,
            Err(e) => assert!(start.elapsed() < LIMIT, "no listener: {e}"),
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn within_limit<T>(fut: impl std::future::Future<Output = std::io::Result<T>>) -> T {
    tokio::time::timeout(LIMIT, fut)
        .await
        .expect("timed out")
        .unwrap()
}

fn free_tcp_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

async fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let start = tokio::time::Instant::now();
    while !cond() {
        assert!(start.elapsed() < LIMIT, "timed out after {LIMIT:?}: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
