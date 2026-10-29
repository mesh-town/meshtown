//! `exit.interface` in the `/api/state` JSON: `null` for any network, `{name, state}` otherwise, with the
//! state in lowercase.

use super::*;
use crate::iface_bind::{ExitIfaceInfo, IfaceState};
use serde_json::json;

mod exit_interface_serializes {
    use super::*;

    #[test]
    fn as_null_for_any_network() {
        // given: a snapshot whose exit has no interface
        let snapshot = snapshot_with(None);
        // when
        let json = to_json(&snapshot);
        // then: the key is present and null
        assert_eq!(json["exit"]["interface"], Value::Null);
        assert!(json["exit"].as_object().unwrap().contains_key("interface"));
    }

    #[test]
    fn as_name_and_state_when_set() {
        // given
        let snapshot = snapshot_with(Some(("wg0", IfaceState::Up)));
        // when
        let json = to_json(&snapshot);
        // then
        assert_eq!(
            json["exit"]["interface"],
            json!({"name": "wg0", "state": "up"})
        );
    }

    #[test]
    fn with_every_state_in_lowercase() {
        for (state, text) in [
            (IfaceState::Up, "up"),
            (IfaceState::Down, "down"),
            (IfaceState::Missing, "missing"),
            (IfaceState::Unsupported, "unsupported"),
        ] {
            // given / when
            let json = to_json(&snapshot_with(Some(("wg0", state))));
            // then
            assert_eq!(json["exit"]["interface"]["state"], text);
        }
    }
}

fn snapshot_with(interface: Option<(&str, IfaceState)>) -> NodeSnapshot {
    let mut snapshot = NodeSnapshot::default();
    snapshot.exit.interface = interface.map(|(name, state)| ExitIfaceInfo {
        name: name.to_string(),
        state,
    });
    snapshot
}
