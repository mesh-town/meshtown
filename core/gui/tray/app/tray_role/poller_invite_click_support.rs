//! Helpers for the invite click tests (`poller_invite_reach_tests.rs`, `poller_invite_notice_tests.rs`): a stub node
//! with real HTTP, a `Poller` whose outputs are recorded, and one-click `setup`. The module is a child of
//! `poller_invite_tests.rs`, so it sees its `SECRET`.

use super::*;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::tray_role::start_fail::StartFail;
use crate::gui::tray::notify::Notice;

/// A node reply of 200 with a ready body.
pub(super) fn ok(body: Value) -> Result<Reply, ApiError> {
    Ok(Reply::Ok(body))
}

/// Body of a node reply with a `reach` field; the number of holders is consistent with the word.
pub(super) fn node_says(reach: &str) -> Value {
    let holders = match reach {
        "issuer_only" => 0,
        "issuer_and_one" => 1,
        _ => 2,
    };
    json!({"invite": SECRET, "holders": holders, "reach": reach})
}

/// Body of an old node's reply: neither `holders` nor `reach`.
pub(super) fn old_node() -> Value {
    json!({"invite": SECRET})
}

/// Whether the clipboard accepts the string.
#[derive(Clone, Copy)]
pub(super) enum Clipboard {
    Takes,
    Refuses,
}

/// Everything the poller showed and wrote during one click. `order` is one journal of the menu notes
/// ("note") and the system notifications ("notice") in the order they happened.
pub(super) struct Click {
    pub(super) notes: Vec<InviteNote>,
    pub(super) copied: Vec<String>,
    pub(super) log: Vec<String>,
    pub(super) notices: Vec<Notice>,
    pub(super) order: Vec<&'static str>,
}

/// Shared sinks for poller events in the click test.
#[derive(Clone, Default)]
pub(super) struct Taps {
    notes: Arc<Mutex<Vec<InviteNote>>>,
    copied: Arc<Mutex<Vec<String>>>,
    log: Arc<Mutex<Vec<String>>>,
    notices: Arc<Mutex<Vec<Notice>>>,
    order: Arc<Mutex<Vec<&'static str>>>,
}

/// Preconditions: the node answers 200 with `node_body`, the clipboard accepts or refuses. One click on
/// the item (`copy_invite`) — the result goes into `Click`.
pub(super) async fn setup(node_body: Value, clipboard: Clipboard) -> Click {
    click_on(node_body, clipboard, None).await
}

/// The same click, on the item of the network in slot `slot` (`copy_invite(Some(slot))`).
pub(super) async fn setup_slot(node_body: Value, clipboard: Clipboard, slot: &str) -> Click {
    click_on(node_body, clipboard, Some(slot)).await
}

async fn click_on(node_body: Value, clipboard: Clipboard, slot: Option<&str>) -> Click {
    let port = node_answering(node_body.to_string()).await;
    let taps = Taps::default();
    let mut p = poller_on(port, &taps, clipboard);

    p.copy_invite(slot).await;

    let click = Click {
        notes: taps.notes.lock().unwrap().clone(),
        copied: taps.copied.lock().unwrap().clone(),
        log: taps.log.lock().unwrap().clone(),
        notices: taps.notices.lock().unwrap().clone(),
        order: taps.order.lock().unwrap().clone(),
    };
    click
}

pub(super) fn poller_on(port: u16, taps: &Taps, clipboard: Clipboard) -> Poller {
    let (notes, log, copied) = (taps.notes.clone(), taps.log.clone(), taps.copied.clone());
    let (notices, order) = (taps.notices.clone(), taps.order.clone());
    let note_order = order.clone();
    let copy_to_clipboard_fn: CopyFn = Arc::new(move |text: &str| -> Result<(), &'static str> {
        match clipboard {
            Clipboard::Takes => {
                copied.lock().unwrap().push(text.to_string());
                Ok(())
            }
            Clipboard::Refuses => Err(REASON_CLIP_FAILED),
        }
    });
    Poller {
        port,
        token_path: PathBuf::from("/nonexistent/token"),
        proxy: ProxyState::Off(ProxyOff::DisabledInSettings),
        spawn: Box::new(|| panic!("spawn must not be called")),
        send_view: Box::new(|_v| {}),
        send_exit_menu: Box::new(|_m| {}),
        log: Box::new(move |line| log.lock().unwrap().push(line)),
        last_view: None,
        last_exit_menu: None,
        copy_to_clipboard_fn,
        notify_fn: Arc::new(move |notice: Notice| {
            notices.lock().unwrap().push(notice);
            order.lock().unwrap().push("notice");
        }),
        send_invite_note: Box::new(move |note| {
            notes.lock().unwrap().push(note);
            note_order.lock().unwrap().push("note");
        }),
        send_serve_note: Box::new(|_n| {}),
        notes_visible_until: None,
        start_fail: StartFail::new(Box::new(|| None), Box::new(|| None), Box::new(|_n| {})),
        timing: PollTiming::PROD,
        overview_gate: crate::gui::tray::app::tray_role::overview_poll::OverviewGate::new(),
    }
}

/// A stub node on 127.0.0.1: reads the request on each connection and answers HTTP 200 with `body`.
/// Returns the port; the task lives until the end of the test runtime.
pub(super) async fn node_answering(body: String) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    crate::node::output::spawn(async move {
        while let Ok((mut socket, _addr)) = listener.accept().await {
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request).await;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    port
}
