//! `GET /api/state`: `network.excluded_by` — who removed this node, filled only when an administrator of a
//! Managed network did. The fixture (`setup`, `Net`, `Removal`) is in `state_kind_support_tests.rs`.

use serde_json::Value;

use super::kind_support::{id_full, name, setup, Net, Removal, ROOT};

mod in_a_managed_network {
    use super::*;

    #[tokio::test]
    async fn removed_by_an_admin_names_the_admin_and_the_name_it_goes_by() {
        // given: node 3 was removed by the creator
        let f = setup(Net::managed(3, &[ROOT]).removed(Removal::Votes { by: ROOT }));
        // when
        let state = f.when_state().await;
        // then
        let by = &state["network"]["excluded_by"];
        assert_eq!(by["id"], id_full(ROOT), "{by}");
        assert_eq!(by["name"], name(ROOT), "{by}");
    }

    #[tokio::test]
    async fn removed_by_a_second_admin_names_that_one() {
        // given: node 3 was removed by node 2, who was made an admin
        let f = setup(Net::managed(3, &[ROOT, 2]).removed(Removal::Votes { by: 2 }));
        // when
        let state = f.when_state().await;
        // then
        let by = &state["network"]["excluded_by"];
        assert_eq!(by["id"], id_full(2), "{by}");
        assert_eq!(by["name"], name(2), "{by}");
    }

    #[tokio::test]
    async fn leaving_by_itself_has_no_excluder() {
        // given: node 3 left on its own
        let f = setup(Net::managed(3, &[ROOT]).removed(Removal::Left));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }

    #[tokio::test]
    async fn a_node_still_in_the_network_has_no_excluder() {
        // given
        let f = setup(Net::managed(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }
}

mod in_a_peer_network {
    use super::*;

    #[tokio::test]
    async fn removed_by_votes_has_no_excluder() {
        // given: the others voted node 2 out
        let f = setup(Net::peer(2).removed(Removal::Votes { by: 3 }));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }

    #[tokio::test]
    async fn a_node_still_in_the_network_has_no_excluder() {
        // given
        let f = setup(Net::peer(2));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }
}
