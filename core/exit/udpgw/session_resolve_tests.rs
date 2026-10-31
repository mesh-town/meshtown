//! Name lookups of the gateway run beside the frame loop. Lookups are scripted per name by `answer` (the loopback
//! echo is closed to a real lookup by the address guard, so a scripted answer is the only way to reach it).

use super::pending::PENDING_FRAMES;
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
    let script = Arc::new(Script {
        gate: watch::channel(open).0,
        calls: AtomicUsize::new(0),
        found: found.map(|addr| (addr, true)),
    });
    let mut all = SCRIPTS.lock().expect("scripts");
    all.push((name.to_string(), script.clone()));
    script
}

fn to_name(conn_id: u16, name: &str, data: &[u8]) -> Frame {
    Frame {
        flags: FLAG_DATA,
        conn_id,
        target: Some(Target::Domain(name.into(), 9)),
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
    let _hang = script("hang-7.test", false, Some(echo));
    client.send(&to_name(1, "hang-7.test", b"a")).await;
    settle().await;
    drop(client);
    let ended = timeout(Duration::from_secs(1), task).await;
    assert!(ended.expect("ended in a second").is_ok());
}
