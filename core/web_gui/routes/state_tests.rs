//! Tests for inserting `node.invite_join` into the `/api/state` response (`apply_state_extras`): the field is always present,
//! with no wait it is `null`, while waiting and on refusal it is an object with the state in lowercase.

use serde_json::{json, Value};

use super::{apply_state_extras, StateExtras};
use crate::web_gui::{InviteJoinState, InviteJoinView};

mod node_not_waiting_for_an_invite {
    use super::*;

    #[test]
    fn invite_join_is_null() {
        // given
        let mut value = snapshot_json();

        // when
        apply_state_extras(&mut value, extras(None));

        // then
        assert_eq!(value["node"].get("invite_join"), Some(&Value::Null));
    }
}

mod node_waiting_for_participants {
    use super::*;

    #[test]
    fn invite_join_is_an_object_with_all_four_fields() {
        // given
        let mut value = snapshot_json();

        // when
        apply_state_extras(
            &mut value,
            extras(Some(view(InviteJoinState::Waiting, Some("Anna")))),
        );

        // then
        assert_eq!(
            value["node"]["invite_join"],
            json!({"state": "waiting", "message": "text", "label": "Anna", "expires_unix": 1_800_000_000u64})
        );
    }

    #[test]
    fn label_is_null_when_the_invite_has_none() {
        // given
        let mut value = snapshot_json();

        // when
        apply_state_extras(
            &mut value,
            extras(Some(view(InviteJoinState::Waiting, None))),
        );

        // then
        assert_eq!(
            value["node"]["invite_join"].get("label"),
            Some(&Value::Null)
        );
    }
}

mod invite_refused_by_participants {
    use super::*;

    #[test]
    fn state_is_refused_in_lowercase() {
        // given
        let mut value = snapshot_json();

        // when
        apply_state_extras(
            &mut value,
            extras(Some(view(InviteJoinState::Refused, None))),
        );

        // then
        assert_eq!(value["node"]["invite_join"]["state"], json!("refused"));
    }
}

fn snapshot_json() -> Value {
    json!({"now": 1.0, "node": {"status": "offline"}, "log": {"tail": []}})
}

fn extras(invite_join: Option<InviteJoinView>) -> StateExtras {
    StateExtras {
        port: 7070,
        config_rev: 1,
        identity: "persistent",
        identity_reason: None,
        network: Value::Null,
        invite_join,
    }
}

fn view(state: InviteJoinState, label: Option<&str>) -> InviteJoinView {
    InviteJoinView {
        state,
        message: "text".to_string(),
        label: label.map(str::to_string),
        expires_unix: 1_800_000_000,
    }
}
