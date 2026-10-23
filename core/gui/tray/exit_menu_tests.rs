//! Tests of the rows of the exits submenu: an offline item is dimmed, regardless of the "chosen" mark.

use super::*;
use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitItem};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};

fn secondary_of(checked: bool, online: bool) -> bool {
    let mut menu = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    menu.items.push(ExitItem {
        id_full: "bb22full".to_string(),
        label: "Sedge".to_string(),
        checked,
        online,
    });
    exit_rows(&menu)[0].secondary
}

#[test]
fn offline_item_is_secondary() {
    assert!(secondary_of(false, false));
}

#[test]
fn selected_offline_item_is_secondary() {
    assert!(secondary_of(true, false));
}

#[test]
fn online_item_is_default() {
    assert!(!secondary_of(false, true));
}
