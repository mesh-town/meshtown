//! Tests of the publishing side: the permission check `admit_port_request`, the local connection `connect_loopback_port` and the connection accept loop
//! of `PortServer` on `NetStack` over `MemNet`. Stream parsing is in `server_stream_tests.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::transport::net::facade::test_support::{pid, secret, within, Probe};
use crate::transport::net::{
    MemNet, NetStack, NetStackBuilder, PeerConn, PeerId, ServiceHandler, PORT,
};
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::*;

fn me() -> PeerId {
    pid(9)
}

#[test]
fn admit_refuses_non_member() {
    assert_eq!(
        admit_port_request(false, Some(&Audience::All), &me()),
        Err(PORT_CODE_REFUSED)
    );
}

#[test]
fn admit_not_published() {
    assert_eq!(
        admit_port_request(true, None, &me()),
        Err(PORT_CODE_NOT_PUBLISHED)
    );
}

#[test]
fn admit_nodes_lets_listed_peer() {
    let a = Audience::Nodes(vec![me()]);
    assert_eq!(admit_port_request(true, Some(&a), &me()), Ok(()));
}

#[test]
fn admit_nodes_refuses_unlisted_peer() {
    let a = Audience::Nodes(vec![pid(8)]);
    assert_eq!(
        admit_port_request(true, Some(&a), &me()),
        Err(PORT_CODE_REFUSED)
    );
}

#[test]
fn admit_empty_nodes_refuses() {
    let a = Audience::Nodes(vec![]);
    assert_eq!(
        admit_port_request(true, Some(&a), &me()),
        Err(PORT_CODE_REFUSED)
    );
}

#[test]
fn admit_nodes_with_repeats_lets_listed() {
    let a = Audience::Nodes(vec![me(), me()]);
    assert_eq!(admit_port_request(true, Some(&a), &me()), Ok(()));
}

#[test]
fn admit_listed_non_member_refused() {
    let a = Audience::Nodes(vec![me()]);
    assert_eq!(
        admit_port_request(false, Some(&a), &me()),
        Err(PORT_CODE_REFUSED)
    );
}

#[test]
fn admit_non_member_unpublished_refused() {
    assert_eq!(
        admit_port_request(false, None, &me()),
        Err(PORT_CODE_REFUSED)
    );
}

#[test]
fn admit_all_is_ok() {
    assert_eq!(
        admit_port_request(true, Some(&Audience::All), &me()),
        Ok(())
    );
}

#[tokio::test]
async fn connect_local_v4() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    assert!(connect_loopback_port(port).await.is_some());
}

#[tokio::test]
async fn connect_local_nobody_listens() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let t = Instant::now();
    assert!(connect_loopback_port(port).await.is_none());
    assert!(t.elapsed() <= LOCAL_CONNECT_TIMEOUT + Duration::from_millis(500));
}

#[tokio::test]
async fn connect_local_v6_fallback() {
    let Ok(l) = TcpListener::bind("[::1]:0").await else {
        return;
    };
    let port = l.local_addr().unwrap().port();
    assert!(connect_loopback_port(port).await.is_some());
}

mod connection {
    use super::*;

    #[tokio::test]
    async fn handler_ends_after_conn_close() {
        // given: the ports handler serves the connection that came to B from A (handed over by Probe)
        let (net, probe) = (MemNet::new(), Probe::new());
        let (from_a, _stacks) = connected(&net, probe.clone()).await;
        let idle = port_service(Members::default(), PublishedPortsTable::new(Vec::new()));
        let mut handler = idle.handle(probe.next().await);
        let early = timeout(Duration::from_millis(100), &mut handler).await;
        assert!(
            early.is_err(),
            "while the connection is alive, the accept loop does not end"
        );

        // when: A closes the connection
        from_a.close(0, b"x");

        // then: the accept loop has ended
        within("handler finished", handler).await;
    }
}

// assumption: `port_service(members, table) -> Arc<dyn ServiceHandler>` stays in `server.rs`.

/// Node `seed` on network `net` with handler `handler` on `PORT`.
fn node(net: &MemNet, seed: u8, handler: Arc<dyn ServiceHandler>) -> NetStack {
    NetStackBuilder::new(secret(seed))
        .transport(net.transport(&secret(seed)))
        .serve(PORT, handler)
        .build()
        .expect("stack was built")
}

/// A (seed 1) learns about B (seed 2, `handler` on `PORT`) and connects to it; the stacks are returned to keep them alive.
async fn connected(
    net: &MemNet,
    handler: Arc<dyn ServiceHandler>,
) -> (Arc<dyn PeerConn>, [NetStack; 2]) {
    let (a, b) = (node(net, 1, Probe::new()), node(net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    let conn = within("A connects to B", a.connect(pid(2), PORT)).await;
    (conn.expect("connect A→B succeeded"), [a, b])
}
