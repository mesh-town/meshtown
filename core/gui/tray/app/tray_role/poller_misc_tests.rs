//! Poller tests: individual cases — a busy port, a foreign token, a disabled web page, a blocked main thread (`poller.rs`).

use super::*;
use super::test_support::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app::exit_menu::disabled_exit_menu;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use crate::app::local_api::tests::{start_fake, temp_dir};

#[tokio::test(flavor = "multi_thread")]
async fn poller_port_busy() {
    let dir = temp_dir("port-busy");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt as _;
        loop {
            if let Ok((mut s, _)) = listener.accept().await {
                let _ = s.write_all(b"SSH-2.0-x\r\n").await;
            }
        }
    });
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    rig.probe.log("held by something other than a node");
    rig.probe.view("PortBusy+Unknown", |v| {
        v.page == PageState::PortBusy(port) && v.node == NodeState::Unknown
    });
    server.abort();
    let _ = server.await;
    rig.probe.view("Absent", is_absent);
    rig.quit();
    assert_eq!(rig.code().await, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_bad_token() {
    let node_dir = temp_dir("bad-token-node");
    let tray_dir = temp_dir("bad-token-tray");
    std::fs::write(tray_dir.join("web-token"), "OTHERtoken\n").unwrap();
    let (handle, _node) = start_fake(0, &node_dir).await;
    let port = handle.shared.port();
    let mut rig = Rig::start(port, tray_dir.join("web-token"), true, spawn_panics());
    rig.probe.log("HTTP 401");
    rig.probe
        .view("PortBusy", |v| v.page == PageState::PortBusy(port));
    rig.quit();
    assert_eq!(rig.code().await, 0);
    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&node_dir);
    let _ = std::fs::remove_dir_all(&tray_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_disabled() {
    let dir = temp_dir("disabled");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepts = Arc::new(AtomicUsize::new(0));
    let counted = accepts.clone();
    let server = tokio::spawn(async move {
        loop {
            if listener.accept().await.is_ok() {
                counted.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    let mut rig = Rig::start(port, dir.join("web-token"), false, spawn_panics());
    let disabled = grey_view(PageState::Disabled, NodeState::Unknown);
    rig.probe.view("Disabled+Unknown", |v| *v == disabled);
    rig.cmd(NodeCmd::Start);
    rig.cmd(NodeCmd::Stop);
    tokio::time::sleep(Duration::from_millis(500)).await;
    rig.probe.drain();
    let views: Vec<View> = rig
        .probe
        .history
        .iter()
        .filter_map(Seen::clone_view)
        .collect();
    assert_eq!(views.last(), Some(&disabled), "the view did not change");
    assert!(
        !rig.probe.logged("node started"),
        "Start starts nothing"
    );
    assert_eq!(accepts.load(Ordering::SeqCst), 0, "no requests");
    rig.quit();
    assert_eq!(rig.code().await, 0);
    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

impl Seen {
    fn clone_view(&self) -> Option<View> {
        match self {
            Seen::View(v) => Some(*v),
            Seen::Log(_) | Seen::Exits(_) | Seen::Invite(_) | Seen::Serve(_) | Seen::StartFail(_) => None,
        }
    }
}

/// A grey view (the node is not running — the submenu is unavailable) turns the submenu off: `show()` calls
/// `show_exits(disabled_exit_menu(settings address))` on every grey view (`poller_phase.rs::show`).
#[tokio::test(flavor = "multi_thread")]
async fn grey_view_turns_exit_menu_off() {
    let dir = temp_dir("grey-exit-off");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
    let mut rig = Rig::start(port, dir.join("web-token"), false, spawn_panics());
    let disabled = grey_view(PageState::Disabled, NodeState::Unknown);
    // Order matters: `exits()` goes before `view()`. The `show_exits` dedup (see below and
    // `poller_phase.rs::show_exits`) sends `disabled_exit_menu()` already on the first grey tick (`Starting`),
    // before the view reaches `Disabled`, and will not send it again on `Disabled` — the same
    // `ExitMenu` twice in a row. Check `view()` first, and the `Probe` cursor moves forward past the earlier
    // `Seen::Exits`, so the following `exits()` finds no event until the timeout.
    rig.probe
        .exits("turned off", |m| *m == disabled_exit_menu(test_proxy()));
    rig.probe.view("Disabled+Unknown", |v| *v == disabled);
    rig.quit();
    assert_eq!(rig.code().await, 0);
    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A grey view with the proxy turned off in the settings — the submenu is off, there is no address.
#[tokio::test(flavor = "multi_thread")]
async fn grey_view_with_proxy_disabled_has_no_address() {
    let dir = temp_dir("grey-exit-no-proxy");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
    let mut rig = Rig::start_with_proxy(port, dir.join("web-token"), false, spawn_panics(), ProxyState::Off(ProxyOff::DisabledInSettings));
    rig.probe
        .exits("turned off, no proxy", |m| *m == disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings)));
    rig.quit();
    assert_eq!(rig.code().await, 0);
    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same `ExitMenu` in two snapshots in a row (here: several ticks of the same grey view) —
/// goes into the channel once, dedup by `last_exit_menu` (`poller_phase.rs::show_exits`).
#[tokio::test(flavor = "multi_thread")]
async fn repeated_same_exit_menu_sent_once() {
    let dir = temp_dir("grey-exit-dedup");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
    let mut rig = Rig::start(port, dir.join("web-token"), false, spawn_panics());
    rig.probe
        .exits("turned off", |m| *m == disabled_exit_menu(test_proxy()));
    // Several poller ticks (poll_interval = 100ms) with the same grey view — `show()` calls
    // `show_exits(disabled_exit_menu())` on each, but dedup must keep a second write out of the channel.
    tokio::time::sleep(Duration::from_millis(500)).await;
    rig.probe.drain();
    let off_count = rig
        .probe
        .history
        .iter()
        .filter(|s| matches!(s, Seen::Exits(m) if *m == disabled_exit_menu(test_proxy())))
        .count();
    assert_eq!(off_count, 1, "a repeat of the same ExitMenu must not go into the channel a second time");
    rig.quit();
    assert_eq!(rig.code().await, 0);
    server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poller_runs_while_main_blocked() {
    let dir = temp_dir("main-blocked");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    // The test thread stands in a synchronous wait, like the main thread in the tray loop.
    let deadline = std::time::Instant::now() + WAIT;
    let ready = loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rig.probe.rx.recv_timeout(left) {
            Ok(Seen::View(v)) if is_ready(&v) => break true,
            Ok(_) => continue,
            Err(_) => break false,
        }
    };
    assert!(ready, "Ready arrived while the test thread is blocked");
    rig.quit();
    assert_eq!(rig.code().await, 0);
    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}
