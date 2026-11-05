//! Name lookups of the gateway run beside the frame loop. Lookups are scripted per name by `answer` (the loopback
//! echo is closed to a real lookup by the address guard, so a scripted answer is the only way to reach it).

use super::pending::{MAX_RESOLVING, PENDING_FRAMES};
use super::*;
use crate::exit::udpgw::codec::Target;
use crate::exit::udpgw::test_support::{cfg_with, spawn_echo, start, Client};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::watch;
use tokio::time::timeout;

/// A scripted name: its lookup waits for the gate, then answers `found`.
struct Script {
    gate: watch::Sender<bool>,
    calls: AtomicUsize,
    found: Option<(SocketAddr, bool)>,
}

static SCRIPTS: Mutex<Vec<(String, Arc<Script>)>> = Mutex::new(Vec::new());

/// The scripted answer for `name`, or `None` if the name is not scripted.
pub(super) async fn answer(name: &str) -> Option<Option<(SocketAddr, bool)>> {
    let script = {
        let all = SCRIPTS.lock().expect("scripts");
        all.iter().find(|(n, _)| n == name)?.1.clone()
    };
    script.calls.fetch_add(1, Ordering::SeqCst);
    let mut gate = script.gate.subscribe();
    gate.wait_for(|open| *open).await.ok()?;
    Some(script.found)
}

fn script(name: &str, open: bool, found: Option<SocketAddr>) -> Arc<Script> {
    script_with(name, open, found.map(|addr| (addr, true)))
}

fn script_with(name: &str, open: bool, found: Option<(SocketAddr, bool)>) -> Arc<Script> {
    let script = Arc::new(Script {
        gate: watch::channel(open).0,
        calls: AtomicUsize::new(0),
        found,
    });
    let mut all = SCRIPTS.lock().expect("scripts");
    all.push((name.to_string(), script.clone()));
    script
}

fn to_name(conn_id: u16, name: &str, data: &[u8]) -> Frame {
    to_name_port(conn_id, name, 9, data)
}

fn to_name_port(conn_id: u16, name: &str, port: u16, data: &[u8]) -> Frame {
    Frame {
        flags: FLAG_DATA,
        conn_id,
        target: Some(Target::Domain(name.into(), port)),
        data: data.to_vec(),
    }
}

async fn fixture() -> (Client, JoinHandle<()>, SocketAddr) {
    let echo = spawn_echo().await;
    let cfg = cfg_with(HashMap::from([("echo.test".to_string(), echo)]));
    let (client, task) = start(cfg, LIMITS);
    (client, task, echo)
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(150)).await;
}

#[tokio::test]
async fn slow_name_does_not_hold_other_sessions() {
    let (mut client, _task, echo) = fixture().await;
    let _hang = script("hang-1.test", false, Some(echo));
    client.send(&to_name(1, "hang-1.test", b"a")).await;
    client.send(&to_name(2, "echo.test", b"b")).await;
    let reply = timeout(Duration::from_secs(1), client.next()).await;
    assert_eq!(
        reply.expect("in a second"),
        Frame::data(2, echo, b"b".to_vec())
    );
}

#[tokio::test]
async fn frames_of_one_session_keep_order_while_its_name_resolves() {
    let (mut client, _task, echo) = fixture().await;
    let slow = script("order-2.test", false, Some(echo));
    for data in [b"1", b"2", b"3"] {
        client.send(&to_name(1, "order-2.test", data)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    slow.gate.send_replace(true);
    for data in [b"1", b"2", b"3"] {
        assert_eq!(client.next().await, Frame::data(1, echo, data.to_vec()));
    }
}

#[tokio::test]
async fn close_while_resolving_drops_its_queue() {
    let (mut client, _task, echo) = fixture().await;
    let gate = script("close-3.test", false, Some(echo));
    client.send(&to_name(1, "close-3.test", b"a")).await;
    settle().await;
    client
        .send(&Frame {
            flags: FLAG_CLOSE,
            conn_id: 1,
            target: None,
            data: vec![],
        })
        .await;
    settle().await;
    assert_eq!(
        gate.gate.receiver_count(),
        0,
        "lookup task ends with its queue"
    );
    gate.gate.send_replace(true);
    assert!(client.silent().await);
    client.send(&to_name(1, "echo.test", b"b")).await;
    assert_eq!(client.next().await, Frame::data(1, echo, b"b".to_vec()));
}

#[tokio::test]
async fn queue_over_the_cap_drops_extra_frames() {
    let (mut client, _task, echo) = fixture().await;
    let gate = script("cap-4.test", false, Some(echo));
    for n in 0..20u8 {
        client.send(&to_name(1, "cap-4.test", &[n])).await;
    }
    settle().await;
    gate.gate.send_replace(true);
    for n in 0..PENDING_FRAMES as u8 {
        assert_eq!(client.next().await, Frame::data(1, echo, vec![n]));
    }
    assert!(client.silent().await);
}

#[tokio::test]
async fn failed_name_drops_its_queue_and_others_flow() {
    let (mut client, _task, echo) = fixture().await;
    let _gone = script("gone-5.test", true, None);
    client.send(&to_name(1, "gone-5.test", b"a")).await;
    client.send(&to_name(2, "echo.test", b"b")).await;
    assert_eq!(client.next().await, Frame::data(2, echo, b"b".to_vec()));
    assert!(client.silent().await);
}

#[tokio::test]
async fn same_name_skips_the_resolver_on_later_frames() {
    let (mut client, _task, echo) = fixture().await;
    let same = script("same-6.test", true, Some(echo));
    client.send(&to_name(1, "same-6.test", b"0")).await;
    client.next().await;
    for n in 1..5u8 {
        client.send(&to_name(1, "same-6.test", &[b'0' + n])).await;
        client.next().await;
    }
    assert_eq!(same.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stream_close_ends_pending_resolves() {
    let (mut client, task, echo) = fixture().await;
    let hang = script("hang-7.test", false, Some(echo));
    client.send(&to_name(1, "hang-7.test", b"a")).await;
    settle().await;
    drop(client);
    let ended = timeout(Duration::from_secs(1), task).await;
    assert!(ended.expect("ended in a second").is_ok());
    settle().await;
    assert_eq!(
        hang.gate.receiver_count(),
        0,
        "lookup task ends with its stream"
    );
}

#[tokio::test]
async fn new_name_of_a_live_session_is_looked_up() {
    let (mut client, _task, echo) = fixture().await;
    let echo_b = spawn_echo().await;
    let a = script("a-8.test", true, Some(echo));
    let b = script("b-8.test", true, Some(echo_b));
    client.send(&to_name(1, "a-8.test", b"a")).await;
    assert_eq!(client.next().await, Frame::data(1, echo, b"a".to_vec()));
    client.send(&to_name(1, "b-8.test", b"b")).await;
    assert_eq!(client.next().await, Frame::data(1, echo_b, b"b".to_vec()));
    assert_eq!(b.calls.load(Ordering::SeqCst), 1);
    client.send(&to_name_port(1, "b-8.test", 10, b"c")).await;
    assert_eq!(client.next().await, Frame::data(1, echo_b, b"c".to_vec()));
    assert_eq!(b.calls.load(Ordering::SeqCst), 2);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn resolved_address_closed_to_exit_drops_its_queue() {
    let (mut client, _task, echo) = fixture().await;
    let _closed = script_with("closed-9.test", true, Some((echo, false)));
    client.send(&to_name(1, "closed-9.test", b"a")).await;
    client.send(&to_name(1, "closed-9.test", b"b")).await;
    client.send(&to_name(2, "echo.test", b"c")).await;
    assert_eq!(client.next().await, Frame::data(2, echo, b"c".to_vec()));
    assert!(client.silent().await);
}

#[tokio::test]
async fn mesh_name_drops_its_queue_without_lookup() {
    let (mut client, _task, echo) = fixture().await;
    let mesh = script("nas.meshtown", true, Some(echo));
    client.send(&to_name(1, "nas.meshtown", b"a")).await;
    client.send(&to_name(1, "nas.meshtown", b"b")).await;
    client.send(&to_name(2, "echo.test", b"c")).await;
    assert_eq!(client.next().await, Frame::data(2, echo, b"c".to_vec()));
    assert!(client.silent().await);
    assert_eq!(mesh.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn lookups_over_the_cap_drop_new_names() {
    let (mut client, _task, echo) = fixture().await;
    let _busy: Vec<_> = (1..=MAX_RESOLVING as u16)
        .map(|c| script(&format!("busy-{c}.test"), false, Some(echo)))
        .collect();
    for c in 1..=MAX_RESOLVING as u16 {
        client
            .send(&to_name(c, &format!("busy-{c}.test"), b"x"))
            .await;
    }
    settle().await;
    let over = script("over-10.test", true, Some(echo));
    let (late, plain) = (MAX_RESOLVING as u16 + 1, MAX_RESOLVING as u16 + 2);
    client.send(&to_name(late, "over-10.test", b"y")).await;
    client.send(&to_name(plain, "echo.test", b"z")).await;
    assert_eq!(client.next().await, Frame::data(plain, echo, b"z".to_vec()));
    assert!(client.silent().await);
    assert_eq!(over.calls.load(Ordering::SeqCst), 0);
}
