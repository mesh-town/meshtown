//! `GET /api/state`: what the page learns about this node's powers in the network —
//! `network.me_role/admins/root/can_*`, `peers[].role` and the `me_admin/can_invite` of `nets[]`.
//! Removal of this node (`network.excluded_by`) is in `network_block_excluded_tests.rs`; the fixture is in
//! `network_block_support_tests.rs`. The tests go through the real router over `FakeHost`, whose node answers
//! `NodeControl::net_roles` with the roles made by `NetRoles::of` from the verdicts of a real tally.

use serde_json::{json, Value};

use super::block_support::{id_full, setup, Net, ROOT};

mod admin {
    use super::*;

    #[tokio::test]
    async fn the_creator_is_root_and_admin() {
        // given
        let f = setup(Net::new(ROOT, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["root"], id_full(ROOT), "{net}");
        assert_eq!(net["me_role"], "admin", "{net}");
        assert_eq!(net["admins"], json!([id_full(ROOT)]), "{net}");
        assert_eq!(net["excluded_by"], Value::Null, "{net}");
    }

    #[tokio::test]
    async fn the_network_lists_its_admins_and_its_creator() {
        // given: the creator and one more admin
        let f = setup(Net::new(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        let admins = net["admins"].as_array().expect("admins is a list");
        assert_eq!(admins.len(), 2, "{net}");
        assert!(admins.contains(&json!(id_full(ROOT))), "{net}");
        assert!(admins.contains(&json!(id_full(2))), "{net}");
        assert_eq!(net["root"], id_full(ROOT), "{net}");
    }

    #[tokio::test]
    async fn one_of_two_admins_may_do_everything() {
        // given
        let f = setup(Net::new(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        assert_flags(&state["network"], [true; 5]);
    }

    #[tokio::test]
    async fn a_granted_admin_has_admin_abilities() {
        // given
        let f = setup(Net::new(2, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["me_role"], "admin", "{net}");
        assert_eq!(
            net["root"],
            id_full(ROOT),
            "the creator stays the root: {net}"
        );
        assert_flags(net, [true; 5]);
    }

    #[tokio::test]
    async fn every_row_names_a_role() {
        // given: nodes 1 and 2 are admins, node 3 is a member
        let f = setup(Net::new(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        let roles: Vec<(String, String)> = state["peers"]
            .as_array()
            .expect("peers is a list")
            .iter()
            .map(|p| (text(&p["id_full"]), text(&p["role"])))
            .collect();
        let want = [(1, "admin"), (2, "admin"), (3, "member")]
            .map(|(n, role)| (id_full(n), role.to_string()));
        assert_eq!(roles, want);
    }

    #[tokio::test]
    async fn the_brief_says_admin_and_lets_this_node_invite() {
        // given
        let f = setup(Net::new(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        let brief = &state["nets"][0];
        assert_eq!(brief["me_admin"], true, "{brief}");
        assert_eq!(brief["can_invite"], true, "{brief}");
    }

    mod the_last_one {
        use super::*;

        #[tokio::test]
        async fn may_not_leave() {
            // given: the only admin
            let f = setup(Net::new(ROOT, &[ROOT]));
            // when
            let state = f.when_state().await;
            // then
            assert_eq!(state["network"]["can_leave"], false, "{state}");
        }

        #[tokio::test]
        async fn may_still_invite_remove_set_admins_and_delete() {
            // given
            let f = setup(Net::new(ROOT, &[ROOT]));
            // when
            let state = f.when_state().await;
            // then
            assert_flags(&state["network"], [true, true, true, true, false]);
        }
    }
}

mod member {
    use super::*;

    #[tokio::test]
    async fn has_the_member_role() {
        // given
        let f = setup(Net::new(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["me_role"], "member", "{net}");
        assert_eq!(net["admins"], json!([id_full(ROOT)]), "{net}");
        assert_eq!(net["excluded_by"], Value::Null, "{net}");
    }

    #[tokio::test]
    async fn a_member_has_no_admin_abilities() {
        // given
        let f = setup(Net::new(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        assert_flags(&state["network"], [false, false, false, false, true]);
    }

    #[tokio::test]
    async fn the_brief_says_not_admin_and_no_invite() {
        // given
        let f = setup(Net::new(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let brief = &state["nets"][0];
        assert_eq!(brief["me_admin"], false, "{brief}");
        assert_eq!(brief["can_invite"], false, "{brief}");
    }
}

// --- helpers ---

/// `can_invite, can_remove, can_set_admins, can_delete_network, can_leave`, in this order.
fn assert_flags(net: &Value, want: [bool; 5]) {
    let names = [
        "can_invite",
        "can_remove",
        "can_set_admins",
        "can_delete_network",
        "can_leave",
    ];
    let got: Vec<Value> = names.iter().map(|k| net[*k].clone()).collect();
    let want: Vec<Value> = want.iter().map(|&b| Value::Bool(b)).collect();
    assert_eq!(got, want, "{names:?} of {net}");
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}
