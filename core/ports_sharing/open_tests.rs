//! Listener tests: binding, reconciling, closing, forwarding a refusal.

use std::time::Duration;

use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::{MemNet, NetStackBuilder, PeerId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::*;
use crate::node::output::{log_page, log_summary};
use crate::ports_sharing::types::Proto;

fn node(n: u8) -> PeerId {
    pid(n)
}

fn op(n: u8, local: u16) -> Opened {
    Opened {
        proto: Proto::Tcp,
        node: node(n),
        port: 80,
        local,
    }
}

// assumption: `Opener::new(NetStack, Members)`; a single-node stack on `MemNet` without neighbors.
async fn opener() -> Opener {
    let net = MemNet::new();
    let stack = NetStackBuilder::new(secret(4))
        .transport(net.transport(&secret(4)))
        .build()
        .expect("single-node stack was built");
    Opener::new(stack, Members::default())
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn state_of(o: &Opener, local: u16) -> OpenState {
    o.bind_states()
        .into_iter()
        .find(|(x, _, _)| x.local == local)
        .unwrap()
        .1
}

async fn connects(local: u16) -> bool {
    TcpStream::connect(("127.0.0.1", local)).await.is_ok()
}

async fn closed_soon(local: u16) -> bool {
    for _ in 0..20 {
        if !connects(local).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

async fn eof_within_1s(mut c: TcpStream) {
    let mut b = [0u8; 8];
    let r = tokio::time::timeout(Duration::from_secs(1), c.read(&mut b))
        .await
        .expect("no close");
    assert!(matches!(r, Ok(0) | Err(_)));
}

#[tokio::test]
async fn opens_free_port() {
    let (o, l) = (opener().await, free_port());
    o.reconcile_listeners(vec![op(1, l)]);
    assert_eq!(state_of(&o, l), OpenState::Listening);
    assert!(connects(l).await);
}

#[tokio::test]
async fn busy_port_bind_failed() {
    let other = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let l = other.local_addr().unwrap().port();
    let o = opener().await;
    o.reconcile_listeners(vec![op(1, l)]);
    match state_of(&o, l) {
        OpenState::BindFailed(t) => assert!(!t.is_empty()),
        s => panic!("{s:?}"),
    }
    assert!(std::net::TcpStream::connect(("127.0.0.1", l)).is_ok());
}

#[tokio::test]
async fn removed_port_closed() {
    let (o, l) = (opener().await, free_port());
    o.reconcile_listeners(vec![op(1, l)]);
    o.reconcile_listeners(vec![]);
    assert!(closed_soon(l).await);
}

#[tokio::test]
async fn changed_local_moves() {
    let (o, l1, l2) = (opener().await, free_port(), free_port());
    o.reconcile_listeners(vec![op(1, l1)]);
    o.reconcile_listeners(vec![op(1, l2)]);
    assert!(closed_soon(l1).await);
    assert_eq!(state_of(&o, l2), OpenState::Listening);
    assert!(connects(l2).await);
}

#[tokio::test]
async fn close_all_closes_all() {
    let (o, l1, l2) = (opener().await, free_port(), free_port());
    o.reconcile_listeners(vec![op(1, l1), op(2, l2)]);
    o.close_all();
    assert!(closed_soon(l1).await && closed_soon(l2).await);
    assert!(o.bind_states().is_empty());
}

#[tokio::test]
async fn close_all_is_final() {
    let (o, l) = (opener().await, free_port());
    o.close_all();
    o.reconcile_listeners(vec![op(1, l)]);
    assert!(o.bind_states().is_empty());
    assert!(std::net::TcpListener::bind(("127.0.0.1", l)).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn rebind_same_local_at_once() {
    let (o, l) = (opener().await, free_port());
    o.reconcile_listeners(vec![op(1, l)]);
    o.reconcile_listeners(vec![]);
    o.reconcile_listeners(vec![op(1, l)]);
    assert_eq!(state_of(&o, l), OpenState::Listening);
    o.reconcile_listeners(vec![op(2, l)]);
    assert_eq!(state_of(&o, l), OpenState::Listening);
}

#[tokio::test]
async fn bind_failed_retried() {
    let other = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let l = other.local_addr().unwrap().port();
    let o = opener().await;
    o.reconcile_listeners(vec![op(1, l)]);
    assert!(matches!(state_of(&o, l), OpenState::BindFailed(_)));
    drop(other);
    o.reconcile_listeners(vec![op(1, l)]);
    assert_eq!(state_of(&o, l), OpenState::Listening);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn wildcard_busy_is_bind_failed() {
    let other = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let l = other.local_addr().unwrap().port();
    let o = opener().await;
    o.reconcile_listeners(vec![op(1, l)]);
    assert!(matches!(state_of(&o, l), OpenState::BindFailed(_)));
    let Ok(v6) = std::net::TcpListener::bind("[::]:0") else {
        return;
    };
    let l6 = v6.local_addr().unwrap().port();
    o.reconcile_listeners(vec![op(1, l6)]);
    assert!(matches!(state_of(&o, l6), OpenState::BindFailed(_)));
}

#[tokio::test]
async fn local_zero_rejected() {
    let o = opener().await;
    o.reconcile_listeners(vec![op(1, 0)]);
    assert!(matches!(state_of(&o, 0), OpenState::BindFailed(_)));
}

#[tokio::test]
async fn states_follow_input() {
    let (o, a, b) = (opener().await, free_port(), free_port());
    o.reconcile_listeners(vec![op(1, b), op(1, a), op(1, b)]);
    let got: Vec<u16> = o.bind_states().iter().map(|(x, _, _)| x.local).collect();
    assert_eq!(got, vec![b, a]);
}

#[tokio::test]
async fn set_opened_from_plain_thread() {
    let (o, l) = (opener().await, free_port());
    let o2 = o.clone();
    std::thread::spawn(move || o2.reconcile_listeners(vec![op(1, l)]))
        .join()
        .unwrap();
    assert_eq!(state_of(&o, l), OpenState::Listening);
}

#[tokio::test]
async fn offline_node_connection_closed() {
    let (o, l) = (opener().await, free_port());
    o.reconcile_listeners(vec![op(1, l)]);
    let mut c = TcpStream::connect(("127.0.0.1", l)).await.unwrap();
    let _ = c.write_all(b"hello").await;
    eof_within_1s(c).await;
}

#[tokio::test]
async fn closing_listener_drops_live_connections() {
    let l = free_port();
    let tl = TcpListener::from_std({
        let s = std::net::TcpListener::bind(("127.0.0.1", l)).unwrap();
        s.set_nonblocking(true).unwrap();
        s
    })
    .unwrap();
    let listener = spawn_listener(tl, |tcp| async move {
        let _keep = tcp;
        std::future::pending::<()>().await
    });
    let c = TcpStream::connect(("127.0.0.1", l)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    listener.close();
    eof_within_1s(c).await;
}

#[test]
fn changes_new_removed_same_moved() {
    let (a, b) = (op(1, 10), op(1, 11));
    assert_eq!(
        listener_changes(&[], std::slice::from_ref(&a)),
        (vec![], vec![a.clone()])
    );
    assert_eq!(
        listener_changes(std::slice::from_ref(&a), &[]),
        (vec![a.clone()], vec![])
    );
    assert_eq!(
        listener_changes(std::slice::from_ref(&a), std::slice::from_ref(&a)),
        (vec![], vec![])
    );
    assert_eq!(
        listener_changes(std::slice::from_ref(&a), std::slice::from_ref(&b)),
        (vec![a], vec![b])
    );
}

#[test]
fn changes_dedup_keeps_order() {
    let (a, b) = (op(1, 10), op(1, 11));
    let (_, to_bind) = listener_changes(&[], &[b.clone(), a.clone(), b.clone()]);
    assert_eq!(to_bind, vec![b, a]);
}

mod after_the_network_stopped {
    use super::*;

    #[tokio::test]
    async fn the_journal_says_the_network_has_stopped() {
        // given: an opener whose network was closed
        let (o, l, seq) = (opener().await, free_port(), journal_mark());
        o.close_all();
        // when: a list with a port arrives
        o.reconcile_listeners(vec![op(1, l)]);
        // then: the member reads the reason in the [port] lines after the mark
        let lines = port_lines_since(seq);
        assert!(
            lines
                .iter()
                .any(|t| t == "cannot open the port: the network has stopped"),
            "no such line in the [port] journal: {lines:?}"
        );
    }

    #[tokio::test]
    async fn the_text_does_not_mention_a_node() {
        // given
        let (o, l, seq) = (opener().await, free_port(), journal_mark());
        o.close_all();
        // when
        o.reconcile_listeners(vec![op(1, l)]);
        // then: every line with the reason avoids the word
        let reasons: Vec<String> = port_lines_since(seq)
            .into_iter()
            .filter(|t| t.starts_with("cannot open the port"))
            .collect();
        assert!(!reasons.is_empty(), "no refusal line was written");
        for text in reasons {
            assert!(!text.to_lowercase().contains("node"), "{text}");
        }
    }

    #[tokio::test]
    async fn the_port_is_not_listened() {
        // given
        let (o, l) = (opener().await, free_port());
        o.close_all();
        // when
        o.reconcile_listeners(vec![op(1, l)]);
        // then: nothing is bound, the port stays free
        assert!(o.bind_states().is_empty());
        assert!(
            std::net::TcpListener::bind(("127.0.0.1", l)).is_ok(),
            "the port is held by the closed opener"
        );
    }

    fn journal_mark() -> u64 {
        log_summary(0).last_seq
    }

    /// Texts of the `port` journal lines written after `seq`.
    fn port_lines_since(seq: u64) -> Vec<String> {
        log_page(seq, 4000)
            .lines
            .into_iter()
            .filter(|l| l.tag == "port")
            .map(|l| l.text)
            .collect()
    }
}
