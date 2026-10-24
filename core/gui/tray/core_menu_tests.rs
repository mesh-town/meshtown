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
