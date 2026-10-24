//! `GET /api/state`: what the page learns about the kind of the network and about this node's powers in it —
//! `network.kind/me_role/admins/root/can_*`, `peers[].role` and the `kind/me_admin/can_invite` of `nets[]`.
//! Removal of this node (`network.excluded_by`) is in `state_kind_excluded_tests.rs`.

// helper: mount the three files from `network_block.rs`, in this order:
// `#[cfg(test)] #[path = "state_kind_support_tests.rs"] mod kind_support;`
// `#[cfg(test)] #[path = "state_kind_tests.rs"] mod kind_tests;`
// `#[cfg(test)] #[path = "state_kind_excluded_tests.rs"] mod kind_excluded_tests;`
// (`state_kind_support_tests.rs` holds the fixture the other two share).
// The tests go through the real router (`test_app`) over `FakeHost`. The node is `RolesNode`: a `SnapNode` that
// also answers `NodeControl::net_roles`, with the roles made by `NetRoles::of` from real verdicts of a tally.
// assumed: `NetRoles::of(&NetKind, &Verdicts, &Members)` and `NodeControl::net_roles() -> Option<NetRoles>` are the
// seam between the node and the page; `SnapNode` itself has no such answer, hence the wrapper in the support file.

use serde_json::{json, Value};

use super::kind_support::{id_full, setup, Net, NODES, ROOT};

mod peer_network {
    use super::*;

    #[tokio::test]
    async fn is_peer_with_no_admins_and_no_root() {
        // given
        let f = setup(Net::peer(2));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["kind"], "Peer", "{net}");
        assert_eq!(net["admins"], json!([]), "{net}");
        assert_eq!(net["root"], Value::Null, "{net}");
    }

    #[tokio::test]
    async fn lets_everyone_invite_remove_and_leave_but_not_set_admins_or_delete() {
        // given
        let f = setup(Net::peer(2));
        // when
        let state = f.when_state().await;
        // then
        assert_flags(&state["network"], [true, true, false, false, true]);
    }

    #[tokio::test]
    async fn rows_of_peers_have_no_role() {
        // given
        let f = setup(Net::peer(2));
        // when
        let state = f.when_state().await;
        // then
        let peers = state["peers"].as_array().expect("peers is a list");
        assert_eq!(peers.len(), NODES.len(), "{state}");
        for peer in peers {
            assert!(
                peer.get("role").is_none(),
                "a peer network has no roles: {peer}"
            );
        }
    }

    #[tokio::test]
    async fn the_brief_says_peer_and_lets_this_node_invite() {
        // given
        let f = setup(Net::peer(2));
        // when
        let state = f.when_state().await;
        // then
        let brief = &state["nets"][0];
        assert_eq!(brief["kind"], "Peer", "{brief}");
        assert_eq!(brief["me_admin"], false, "{brief}");
        assert_eq!(brief["can_invite"], true, "{brief}");
    }
}

mod managed_admin {
    use super::*;

    #[tokio::test]
    async fn the_creator_sees_a_managed_network_and_the_admin_role() {
        // given
        let f = setup(Net::managed(ROOT, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["kind"], "Managed", "{net}");
        assert_eq!(net["me_role"], "admin", "{net}");
        assert_eq!(net["excluded_by"], Value::Null, "{net}");
    }

    #[tokio::test]
    async fn the_network_lists_its_admins_and_its_creator() {
        // given: the creator and one more admin
        let f = setup(Net::managed(ROOT, &[ROOT, 2]));
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
        let f = setup(Net::managed(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        assert_flags(&state["network"], [true; 5]);
    }

    #[tokio::test]
    async fn an_admin_who_is_not_the_creator_has_the_same_powers() {
        // given
        let f = setup(Net::managed(2, &[ROOT, 2]));
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
        let f = setup(Net::managed(ROOT, &[ROOT, 2]));
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
    async fn the_brief_says_managed_admin_and_lets_this_node_invite() {
        // given
        let f = setup(Net::managed(ROOT, &[ROOT, 2]));
        // when
        let state = f.when_state().await;
        // then
        let brief = &state["nets"][0];
        assert_eq!(brief["kind"], "Managed", "{brief}");
        assert_eq!(brief["me_admin"], true, "{brief}");
        assert_eq!(brief["can_invite"], true, "{brief}");
    }

    mod the_last_one {
        use super::*;

        #[tokio::test]
        async fn may_not_leave() {
            // given: the only admin
            let f = setup(Net::managed(ROOT, &[ROOT]));
            // when
            let state = f.when_state().await;
            // then
            assert_eq!(state["network"]["can_leave"], false, "{state}");
        }

        #[tokio::test]
        async fn may_still_invite_remove_set_admins_and_delete() {
            // given
            let f = setup(Net::managed(ROOT, &[ROOT]));
            // when
            let state = f.when_state().await;
            // then
            assert_flags(&state["network"], [true, true, true, true, false]);
        }
    }
}

mod managed_member {
    use super::*;

    #[tokio::test]
    async fn sees_a_managed_network_and_the_member_role() {
        // given
        let f = setup(Net::managed(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let net = &state["network"];
        assert_eq!(net["kind"], "Managed", "{net}");
        assert_eq!(net["me_role"], "member", "{net}");
        assert_eq!(net["admins"], json!([id_full(ROOT)]), "{net}");
        assert_eq!(net["excluded_by"], Value::Null, "{net}");
    }

    #[tokio::test]
    async fn may_not_invite_remove_set_admins_or_delete_but_may_leave() {
        // given
        let f = setup(Net::managed(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        assert_flags(&state["network"], [false, false, false, false, true]);
    }

    #[tokio::test]
    async fn the_brief_says_managed_not_admin_and_no_invite() {
        // given
        let f = setup(Net::managed(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        let brief = &state["nets"][0];
        assert_eq!(brief["kind"], "Managed", "{brief}");
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
