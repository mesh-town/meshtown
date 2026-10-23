//! The demo's slots seen through the real page: `--nets N`.

use serde_json::json;

use super::tests_support::*;
use crate::args::Args;
use clap::Parser;

mod args {
    use super::*;

    #[test]
    fn nets_defaults_to_one() {
        let args = Args::parse_from(["web_demo"]);

        assert_eq!(args.nets, 1);
    }

    #[test]
    fn nets_outside_one_to_eight_is_refused() {
        for bad in ["0", "9"] {
            let parsed = Args::try_parse_from(["web_demo", "--nets", bad]);

            assert!(parsed.is_err(), "--nets {bad} must not parse");
        }
    }
}

mod without_the_flag {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn the_page_lists_one_main_network() {
        let demo = setup(1).await;

        let (status, body) = demo.get("/api/nets").await;

        assert_eq!(status, 200, "{body}");
        assert_eq!(ids_of(&body["nets"]), ["main"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_host_holds_a_single_main_slot() {
        let demo = setup(1).await;

        assert_eq!(demo.slot_ids(), ["main"]);
    }
}

mod two_nets {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn the_summary_lists_two_networks_main_first() {
        let demo = setup(2).await;

        let (status, body) = demo.get("/api/nets").await;

        assert_eq!(status, 200, "{body}");
        let ids = ids_of(&body["nets"]);
        assert_eq!(ids.len(), 2, "{body}");
        assert_eq!(ids[0], "main");
        assert!(
            is_slot_id(&ids[1]),
            "second id {:?} is not a slot id",
            ids[1]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_networks_carry_the_demo_labels() {
        let demo = setup(2).await;

        let (_, body) = demo.get("/api/nets").await;

        assert_eq!(body["nets"][0]["label"], "demo-network");
        assert_eq!(body["nets"][1]["label"], "demo-network-2");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_overview_has_a_state_for_each_slot() {
        let demo = setup(2).await;

        let (status, body) = demo.get("/api/overview").await;

        assert_eq!(status, 200, "{body}");
        let slots: Vec<String> = body["slots"]
            .as_array()
            .expect("slots array")
            .iter()
            .map(|s| s["slot"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(slots, demo.slot_ids());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn each_network_is_reachable_under_its_own_path() {
        let demo = setup(2).await;
        let second = demo.slot_ids()[1].clone();

        let (status, _) = demo.get(&format!("/api/net/{second}/members")).await;

        assert_eq!(status, 200);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_demo_does_not_exceed_eight_networks() {
        let demo = setup(8).await;

        let (status, body) = demo.post("/api/nets", json!({})).await;

        assert_eq!(status, 409, "{body}");
        assert_eq!(body["error"], "too_many_nets");
    }
}
