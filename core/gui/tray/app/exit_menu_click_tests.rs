//! Tests for `exit_menu_click`: the check marks of the "Exit via…" model after a click.

use super::*;
use crate::gui::tray::app::exit_menu::disabled_exit_menu;
use crate::gui::tray::app::nets_menu::ExitGroup;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};

fn item(id: &str, checked: bool) -> ExitItem {
    ExitItem {
        id_full: id.to_string(),
        label: id.to_string(),
        checked,
        online: true,
    }
}

fn menu(items: Vec<ExitItem>) -> ExitMenu {
    let mut m = disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    m.enabled = true;
    m.items = items;
    m
}

fn exit(id: &str) -> MenuTarget {
    MenuTarget::Exit {
        slot: None,
        id: id.to_string(),
    }
}

fn marks(items: &[ExitItem]) -> Vec<bool> {
    items.iter().map(|i| i.checked).collect()
}

/// "Directly" is derived from the flags: nothing checked in any list.
fn direct(m: &ExitMenu) -> bool {
    m.items.iter().all(|i| !i.checked)
        && m.groups.iter().all(|g| g.items.iter().all(|i| !i.checked))
}

#[test]
fn click_unchecked_exit_makes_it_the_only_checked_and_clears_direct() {
    let m = menu(vec![item("a", false), item("b", false)]);
    assert!(direct(&m));

    let next = m.after_click(&exit("b"));

    assert_eq!(marks(&next.items), vec![false, true]);
    assert!(!direct(&next));
}

#[test]
fn click_checked_exit_clears_choice() {
    let m = menu(vec![item("a", true), item("b", false)]);

    let next = m.after_click(&exit("a"));

    assert_eq!(marks(&next.items), vec![false, false]);
    assert!(direct(&next));
}

#[test]
fn click_other_exit_moves_the_check() {
    let m = menu(vec![item("a", true), item("b", false)]);

    let next = m.after_click(&exit("b"));

    assert_eq!(marks(&next.items), vec![false, true]);
}

#[test]
fn click_directly_clears_every_check() {
    let mut m = menu(vec![item("a", true)]);
    m.groups = vec![ExitGroup {
        slot: "n1".to_string(),
        title: "n1".to_string(),
        items: vec![item("c", true)],
    }];

    let next = m.after_click(&MenuTarget::Directly);

    assert!(direct(&next));
}

#[test]
fn click_in_group_touches_only_that_group() {
    let group = |slot: &str| ExitGroup {
        slot: slot.to_string(),
        title: slot.to_string(),
        items: vec![item("x", false), item("y", true)],
    };
    let mut m = menu(Vec::new());
    m.groups = vec![group("n1"), group("n2")];
    let target = MenuTarget::Exit {
        slot: Some("n1".to_string()),
        id: "x".to_string(),
    };

    let next = m.after_click(&target);

    assert_eq!(marks(&next.groups[0].items), vec![true, false]);
    assert_eq!(marks(&next.groups[1].items), vec![false, true]);
}

#[test]
fn click_of_unknown_id_changes_nothing() {
    let m = menu(vec![item("a", true), item("b", false)]);

    assert_eq!(m.after_click(&exit("zzz")), m);
    assert_eq!(m.after_click(&MenuTarget::Quit), m);
}
