//! Tests that the network submenus and the "Add a network…" item do not depend on a network being online.

use super::{views_from_state_json, ExitMenu};
use crate::gui::tray::app::local_api::stub_node::{
    friends, overview_json, state_json, work, NetSpec, StateSpec,
};
use crate::gui::tray::app::menu_order::has_network;
use crate::gui::tray::app::proxy_state::ProxyState;
use serde_json::Value;

fn plan() -> ProxyState {
    ProxyState::On("127.0.0.1:1080".to_string())
}

fn offline(net: NetSpec<'static>) -> NetSpec<'static> {
    NetSpec {
        status: "offline",
        state: Some(state_json(&StateSpec {
            status: "offline",
            ..Default::default()
        })),
        ..net
    }
}

fn menu_of(summary: &Value) -> ExitMenu {
    views_from_state_json(summary, &plan()).0
}

#[test]
fn two_offline_networks_are_nested_and_can_be_added_to() {
    // given
    let summary = overview_json(&[offline(friends()), offline(work())]);

    // when
    let menu = menu_of(&summary);

    // then
    assert_eq!(menu.nets.len(), 2);
    assert!(has_network(&menu));
}

#[test]
fn two_offline_networks_without_a_state_are_nested_and_can_be_added_to() {
    // given
    let bare = |net: NetSpec<'static>| NetSpec {
        state: None,
        ..offline(net)
    };
    let summary = overview_json(&[bare(friends()), bare(work())]);

    // when
    let menu = menu_of(&summary);

    // then
    assert_eq!(menu.nets.len(), 2);
    assert!(has_network(&menu));
}

#[test]
fn two_offline_networks_whose_state_lacks_the_node_are_nested_and_can_be_added_to() {
    // given
    let hollow = |net: NetSpec<'static>| NetSpec {
        state: Some(serde_json::json!({})),
        ..offline(net)
    };
    let summary = overview_json(&[hollow(friends()), hollow(work())]);

    // when
    let menu = menu_of(&summary);

    // then
    assert_eq!(menu.nets.len(), 2);
    assert!(has_network(&menu));
}

#[test]
fn one_offline_network_can_be_added_to() {
    // given
    let summary = overview_json(&[offline(friends())]);

    // when
    let menu = menu_of(&summary);

    // then
    assert!(has_network(&menu));
}

#[test]
fn a_real_offline_overview_of_two_networks_is_nested_and_can_be_added_to() {
    // given: the reply of a real node with two networks that have not come online
    let summary: Value = serde_json::from_str(include_str!("real_offline_overview.json")).unwrap();

    // when
    let menu = menu_of(&summary);

    // then
    assert_eq!(menu.nets.len(), 2);
    assert!(has_network(&menu));
}
