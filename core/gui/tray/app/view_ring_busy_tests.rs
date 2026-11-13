//! Tests of the orange sector of an overloaded exit: roles from the snapshot and the sector colour in the raster.

use super::*;
use crate::gui::tray::app::view::ICON_SIDE_PX;
use crate::gui::tray::app::view_ring_paint::ring_rgba;
use serde_json::{json, Value};

fn ring_from(route: Value, exit_peer: Value) -> Ring {
    let state = json!({
        "route": route,
        "peers": [{"self": true, "id_full": "me", "online": true}, exit_peer,
                  {"id_full": "p2", "online": true, "overloaded": "limit"}],
    });
    ring_of(&TraySnapshot::from_value(&state))
}

fn exit_route(busy: bool) -> Value {
    json!({"state": "exit", "exits": [{"id_full": "e1", "online": true, "active": true, "busy": busy}]})
}

fn exit_peer(path: &str) -> Value {
    json!({"id_full": "e1", "online": true, "path": path})
}

#[test]
fn busy_active_exit_is_orange_sector() {
    let ring = ring_from(exit_route(true), exit_peer("direct"));
    assert_eq!(ring.count(RingRole::ActiveExitBusy), 1);
    assert_eq!(ring.count(RingRole::ActiveExit), 0);
    assert_eq!(ring.role_at(0), RingRole::ActiveExitBusy);
    assert_eq!(role_rgb(RingRole::ActiveExitBusy), [255, 140, 0]);
}

#[test]
fn busy_relayed_exit_is_dashed_orange() {
    let ring = ring_from(exit_route(true), exit_peer("relay"));
    assert_eq!(ring.count(RingRole::ExitViaRelayBusy), 1);
    assert!(RingRole::ExitViaRelayBusy.is_dashed());
    assert!(!RingRole::ActiveExitBusy.is_dashed());
    assert_eq!(
        role_rgb(RingRole::ExitViaRelayBusy),
        role_rgb(RingRole::ActiveExitBusy)
    );
}

#[test]
fn unavailable_route_stays_red() {
    let route = json!({"state": "unavailable", "exits": [{"id_full": "e1", "busy": true}]});
    let ring = ring_from(route, exit_peer("direct"));
    assert_eq!(ring.count(RingRole::ExitDown), 1);
    assert_eq!(ring.count(RingRole::ActiveExitBusy), 0);
}

#[test]
fn overloaded_non_active_peer_stays_green() {
    let ring = ring_from(exit_route(false), exit_peer("direct"));
    assert_eq!(ring.count(RingRole::Online), 1);
    assert_eq!(ring.count(RingRole::ActiveExit), 1);
    assert_eq!(ring.count(RingRole::ActiveExitBusy), 0);
}

#[test]
fn ring_paint_orange_differs_from_red_and_blue() {
    let mid = |role| {
        let rgba = ring_rgba(&Ring::uniform(role, 1), ICON_SIDE_PX);
        let at = ((18 * ICON_SIDE_PX + 31) * 4) as usize;
        [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
    };
    let orange = mid(RingRole::ActiveExitBusy);
    assert_eq!(orange, [255, 140, 0, 255]);
    assert_ne!(orange, mid(RingRole::ExitDown));
    assert_ne!(orange, mid(RingRole::ActiveExit));
}
