//! Deleting a Managed network, end to end: three nodes (the creator, an administrator and a plain member) run
//! under the process host over the in-process test network; the administrator deletes the network from the page.
//!
//! What it does: every node leaves the network and keeps a notice that the page lists until it is
//! acknowledged; a close asked for by a plain member changes nothing. The network key of every node comes from its
//! settings file, not from a flag: a key given by a flag could not be left.
//! What it does not do: a forged close record in the replica of a member is a unit test of the count; here the
//! page's own refusal stands in for it. Every wait has a ceiling, so a hang fails the test.

// assumed: the nodes are started like `common/nets_host.rs` does, but with the list consulted
// (`open_list = false`) and the Managed kind in `NodeConfig`, which `Relaunch::node_config` leaves off in tests;
// the network key is written into the settings file before the node starts.
// assumed: the leave grace of the host (`node::host::retire::LEAVE_GRACE`) is not exported; it is copied here.
// assumed: the state page carries `network.me_role`, the members page `rows[].id_full` and `status`.

#[path = "common/node_web.rs"]
mod common;
#[path = "common/close_trio.rs"]
mod trio_nodes;

use std::time::Duration;

use serde_json::json;

use common::wait_until;
use trio_nodes::{trio, LABEL};

/// The wait of the host between the leave record and the stop of the node.
const LEAVE_GRACE: Duration = Duration::from_secs(3);
/// How long the notices may take to show up after the close was asked for.
const NOTICE_LIMIT: Duration = Duration::from_secs(LEAVE_GRACE.as_secs() + 5);

mod admin_deletes_the_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn every_node_gets_a_notice_and_leaves() {
        // given: three nodes in one Managed network, none of them has a notice
        let t = trio().await;
        assert!(t.all().all(|n| n.notices().is_empty()), "no notice yet");
        // when: the administrator deletes the network
        let (status, body) = t
            .admin
            .post("/api/network/delete", json!({"confirm": true}));
        // then: each node shows one notice about the deletion by the administrator, within the grace and a margin
        assert_eq!(status, 200, "delete: {body}");
        let noted = wait_until(NOTICE_LIMIT, || t.all().all(|n| n.notices().len() == 1)).await;
        assert!(
            noted,
            "not every node noted the deletion: {:?}",
            t.all_notices()
        );
        assert_eq!(t.root.notices()[0]["label"], LABEL);
        for node in t.all() {
            let notes = node.notices();
            assert_eq!(notes[0]["kind"], "net_deleted", "{notes:?}");
            assert!(notes[0].get("label").is_some(), "{notes:?}");
            assert!(notes[0]["id_hex"].is_string(), "the network id: {notes:?}");
            assert!(notes[0]["id"].is_u64(), "{notes:?}");
            assert!(notes[0]["at_unix"].is_u64(), "{notes:?}");
            assert_eq!(notes[0]["by"], json!(t.admin.id_full()));
        }
        t.teardown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_nodes_are_without_a_network_after_the_grace() {
        // given: the deletion was noted everywhere
        let t = trio().await;
        t.admin
            .post("/api/network/delete", json!({"confirm": true}));
        let noted = wait_until(NOTICE_LIMIT, || t.all().all(|n| !n.notices().is_empty())).await;
        assert!(
            noted,
            "not every node noted the deletion: {:?}",
            t.all_notices()
        );
        // when / then: the nodes are gone from the network, which is checked only after the notices came
        let gone = wait_until(LEAVE_GRACE + Duration::from_secs(5), || {
            t.all().all(|n| n.is_offline())
        })
        .await;
        assert!(
            gone,
            "some node is still in the network: {:?}",
            t.all_status()
        );
        t.teardown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn acknowledging_clears_the_notice() {
        // given: every node has a notice
        let t = trio().await;
        t.admin
            .post("/api/network/delete", json!({"confirm": true}));
        let noted = wait_until(NOTICE_LIMIT, || t.all().all(|n| !n.notices().is_empty())).await;
        assert!(
            noted,
            "not every node noted the deletion: {:?}",
            t.all_notices()
        );
        // when: each node acknowledges its notice
        for node in t.all() {
            let id = node.notices()[0]["id"].clone();
            let (status, body) = node.post("/api/notices/ack", json!({ "id": id }));
            assert_eq!(status, 200, "ack: {body}");
            assert_eq!(body, json!({}));
        }
        // then: the lists are empty
        assert!(
            t.all().all(|n| n.notices().is_empty()),
            "{:?}",
            t.all_notices()
        );
        t.teardown().await;
    }
}

mod plain_member_asks_to_delete {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_network_and_the_nodes_stay() {
        // given: the creator, an administrator and a plain member in one Managed network
        let t = trio().await;
        // when: the plain member asks to delete the network
        let (status, body) = t
            .member
            .post("/api/network/delete", json!({"confirm": true}));
        // then: it is refused (the witness that the request reached the count), and after the grace nothing changed
        assert_eq!(status, 409, "{body}");
        assert_eq!(body["error"], "not_admin");
        tokio::time::sleep(LEAVE_GRACE + Duration::from_secs(2)).await;
        for node in t.all() {
            assert!(node.notices().is_empty(), "{:?}", t.all_notices());
            assert!(!node.is_offline(), "{:?}", t.all_status());
        }
        t.teardown().await;
    }
}
