//! `GET /api/state`: `network.excluded_by` — who removed this node, filled only when an administrator did.
//! `network.excluded_reason` follows the own row. The fixture (`setup`, `Net`, `Removal`) is in `network_block_support_tests.rs`.

use serde_json::Value;

use super::block_support::{id_full, name, setup, Net, Removal, ROOT};

mod excluded_by {
    use super::*;

    #[tokio::test]
    async fn removed_by_an_admin_names_the_admin_and_the_name_it_goes_by() {
        // given: node 3 was removed by the creator
        let f = setup(Net::new(3, &[ROOT]).removed(Removal::Votes { by: ROOT }));
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
        let f = setup(Net::new(3, &[ROOT, 2]).removed(Removal::Votes { by: 2 }));
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
        let f = setup(Net::new(3, &[ROOT]).removed(Removal::Left));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }

    #[tokio::test]
    async fn a_node_still_in_the_network_has_no_excluder() {
        // given
        let f = setup(Net::new(3, &[ROOT]));
        // when
        let state = f.when_state().await;
        // then
        assert_eq!(state["network"]["excluded_by"], Value::Null, "{state}");
    }
}

mod excluded_reason {
    use super::*;

    #[tokio::test]
    async fn excluded_reason_follows_the_own_row() {
        // given / when: node 3 was removed with a note, removed without one, and left with a note on its record
        let noted = setup(
            Net::new(3, &[ROOT])
                .removed(Removal::Votes { by: ROOT })
                .reason("spam"),
        );
        let plain = setup(Net::new(3, &[ROOT]).removed(Removal::Votes { by: ROOT }));
        let left = setup(Net::new(3, &[ROOT]).removed(Removal::Left).reason("spam"));
        let (noted, plain, left) = (
            noted.when_state().await,
            plain.when_state().await,
            left.when_state().await,
        );
        // then: the note only for a removal that has one; `null` otherwise, the key is always there
        assert_eq!(noted["network"]["excluded_reason"], "spam", "{noted}");
        assert_eq!(plain["network"]["excluded_reason"], Value::Null, "{plain}");
        assert_eq!(left["network"]["excluded_reason"], Value::Null, "{left}");
    }

    #[tokio::test]
    async fn a_node_still_in_the_network_has_no_reason() {
        // given / when
        let state = setup(Net::new(3, &[ROOT])).when_state().await;
        // then
        assert_eq!(state["network"]["excluded_reason"], Value::Null, "{state}");
    }

    #[test]
    fn the_key_stands_right_after_excluded_by() {
        // given
        let peer = |n: u8| crate::transport::net::NodeSecret::from_bytes(&[n; 32]).peer_id();
        let roles = crate::web_gui::NetRoles::bare(&peer(1), &peer(2));
        // when
        let keys: Vec<&str> = roles.fields().iter().map(|(k, _)| *k).collect();
        // then
        assert_eq!(&keys[keys.len() - 2..], ["excluded_by", "excluded_reason"]);
    }
}
