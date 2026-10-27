//! Tests of the menu click dispatch (`core_menu.rs`): which items open a page and which go to the node.

use super::super::{channel, TrayCmd};
use super::*;
use std::sync::Mutex;

use super::super::TrayBackend;

static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A backend that records the URLs it was asked to open.
struct RecordingTray;
static RECORDING: RecordingTray = RecordingTray;

impl TrayBackend for RecordingTray {
    fn run(&self, _core: Core) -> ! {
        unreachable!("tests never start the event loop")
    }
    fn wake(&self) {}
    fn open_url(&self, url: &str) -> Result<(), String> {
        OPENED.lock().unwrap().push(url.to_string());
        Ok(())
    }
    fn copy_to_clipboard(&self, _text: &str) -> Result<(), &'static str> {
        Ok(())
    }
    fn notify(&self, _title: &str, _body: &str) -> Result<(), &'static str> {
        Ok(())
    }
}

fn page() -> Option<String> {
    Some("http://127.0.0.1:1/".to_string())
}

/// Clicks `id` and returns the URLs opened and whether any command reached the node.
fn click(id: &str) -> (Vec<String>, bool) {
    let (node_tx, mut node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut core) = channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        page,
        node_tx,
        &RECORDING,
    );
    OPENED.lock().unwrap().clear();
    tx.send(TrayCmd::Menu(id.to_string()));
    assert_eq!(core.drain_commands(), None);
    let opened = OPENED.lock().unwrap().clone();
    (opened, node_rx.try_recv().is_ok())
}

/// Sends `cmd` through a fresh `Core` and returns the commands that reached the node.
fn sent_node_cmds(cmd: TrayCmd) -> Vec<NodeCmd> {
    let (node_tx, mut node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut core) = channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        page,
        node_tx,
        &RECORDING,
    );
    tx.send(cmd);
    assert_eq!(core.drain_commands(), None);
    std::iter::from_fn(|| node_rx.try_recv().ok()).collect()
}

#[test]
fn menu_open_asks_for_snapshot() {
    assert_eq!(sent_node_cmds(TrayCmd::MenuOpened), vec![NodeCmd::Refresh]);
}

#[test]
fn add_and_notice_items_open_the_page_and_send_nothing() {
    let (opened, sent) = click("add");
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].ends_with("peers/add"), "{opened:?}");
    assert!(!sent);
    let (opened, sent) = click("notice:7");
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].ends_with("peers"), "{opened:?}");
    assert!(!sent);
}

#[test]
fn exit_click_updates_remembered_model_before_snapshot() {
    use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitItem};
    use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
    let (node_tx, mut node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_tx, mut core) = channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        page,
        node_tx,
        &RECORDING,
    );
    let item = |id: &str, checked| ExitItem {
        id_full: id.to_string(),
        label: id.to_string(),
        checked,
        online: true,
    };
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.enabled = true;
    menu.items = vec![item("a", true), item("b", false)];
    core.apply_exits(menu);

    core.handle_menu("exit:b");

    let marks: Vec<bool> = core
        .last_exit
        .unwrap()
        .items
        .iter()
        .map(|i| i.checked)
        .collect();
    assert_eq!(marks, vec![false, true]);
    assert!(node_rx.try_recv().is_ok());
}

#[test]
fn directly_click_clears_the_exit_of_the_chosen_network() {
    use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitItem};
    use crate::gui::tray::app::nets_menu::{ExitGroup, NetMenu};
    use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
    let (node_tx, mut node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_tx, mut core) = channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        page,
        node_tx,
        &RECORDING,
    );
    let net = |slot: &str| NetMenu {
        slot: slot.to_string(),
        title: slot.to_string(),
        joining: false,
        online: true,
        peers: Vec::new(),
        peers_more: 0,
        serves_exit: false,
        admitting: 0,
        invite_admins_only: false,
        label: slot.to_string(),
    };
    let group = |slot: &str, checked| ExitGroup {
        slot: slot.to_string(),
        title: slot.to_string(),
        items: vec![ExitItem {
            id_full: "peer".to_string(),
            label: "peer".to_string(),
            checked,
            online: true,
        }],
    };
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.enabled = true;
    menu.nets = vec![net("a"), net("b")];
    menu.groups = vec![group("a", false), group("b", true)];
    core.apply_exits(menu);

    core.handle_menu("exit:");

    assert_eq!(
        node_rx.try_recv().ok(),
        Some(NodeCmd::ClearExits(Some("b".to_string())))
    );
    assert!(node_rx.try_recv().is_err());
    assert!(core.facts.nets.iter().all(|n| !n.exit_chosen));
}
