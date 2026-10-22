//! Tests for `view_sender`: the page opens once per process, only for a ready page without a network.

use std::sync::{Arc, Mutex};

use super::*;
use crate::gui::tray::app::tray_role::views::grey_view;
use crate::gui::tray::app::view::NodeState;

type Log = Arc<Mutex<Vec<String>>>;

fn sender() -> (Box<dyn Fn(View) + Send>, Log) {
    let log: Log = Arc::default();
    let sink = log.clone();
    let send = view_sender(move |cmd| {
        let line = match cmd {
            TrayCmd::Update(_) => "update".to_string(),
            TrayCmd::Menu(id) => format!("menu:{id}"),
            _ => "other".to_string(),
        };
        sink.lock().unwrap().push(line);
    });
    (send, log)
}

fn ready(net: NetLink) -> View {
    View {
        net,
        ..grey_view(PageState::Ready, NodeState::Running)
    }
}

fn lines(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

#[test]
fn no_network_view_triggers_open_once() {
    let (send, log) = sender();
    send(ready(NetLink::NoNetwork));
    send(ready(NetLink::NoNetwork));
    assert_eq!(lines(&log), ["update", "menu:open", "update"]);
}

#[test]
fn joined_view_never_triggers() {
    let (send, log) = sender();
    for net in [NetLink::Joined, NetLink::Connecting, NetLink::Lost] {
        send(ready(net));
    }
    assert_eq!(lines(&log), ["update", "update", "update"]);
}

#[test]
fn grey_startup_views_do_not_trigger() {
    let (send, log) = sender();
    for page in [
        PageState::Starting,
        PageState::Unavailable,
        PageState::Disabled,
        PageState::PortBusy(7777),
    ] {
        send(grey_view(page, NodeState::Starting));
    }
    assert_eq!(lines(&log), ["update"; 4]);
    send(ready(NetLink::NoNetwork));
    assert_eq!(lines(&log).last().map(String::as_str), Some("menu:open"));
}

#[test]
fn open_id_is_the_menu_open_id() {
    let (send, log) = sender();
    send(ready(NetLink::NoNetwork));
    assert_eq!(MENU_OPEN, "open");
    assert_eq!(lines(&log)[1], format!("menu:{MENU_OPEN}"));
}

#[test]
fn latch_survives_a_net_flap() {
    let (send, log) = sender();
    send(ready(NetLink::NoNetwork));
    send(ready(NetLink::Joined));
    send(ready(NetLink::NoNetwork));
    assert_eq!(lines(&log), ["update", "menu:open", "update", "update"]);
}
