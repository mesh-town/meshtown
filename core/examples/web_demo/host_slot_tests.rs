//! The demo's slots seen through the real page: adding a slot, leaving one.

use serde_json::{json, Value};

use super::tests_support::*;

mod adding_a_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn answers_created_with_the_new_slot() {
        let demo = setup(2).await;

        let (status, body) = demo.post("/api/nets", json!({})).await;

        assert_eq!(status, 201, "{body}");
        let slot = body["slot"].as_str().expect("slot id").to_string();
        assert!(is_slot_id(&slot), "{slot:?} is not a slot id");
        assert_eq!(ids_of(&body["nets"]).len(), 3, "{body}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_third_slot_shows_up_in_the_summary() {
        let demo = setup(2).await;

        let (_, added) = demo.post("/api/nets", json!({})).await;

        let (_, body) = demo.get("/api/nets").await;
        let ids = ids_of(&body["nets"]);
        assert_eq!(ids.len(), 3, "{body}");
        assert_eq!(ids[2], added["slot"].as_str().unwrap_or_default());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_new_slot_waits_without_a_network_key() {
        let demo = setup(1).await;

        let (_, added) = demo.post("/api/nets", json!({})).await;

        assert_eq!(added["nets"][1]["id_hex"], Value::Null, "{added}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_added_slots_get_different_ids() {
        let demo = setup(1).await;

        let (_, first) = demo.post("/api/nets", json!({})).await;
        let (_, second) = demo.post("/api/nets", json!({})).await;

        assert_ne!(first["slot"], second["slot"]);
    }
}

mod added_slot_without_a_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn process_fields_are_read_only() {
        let demo = setup(1).await;
        let slot = demo.add_slot().await;

        let (status, config) = demo.get(&format!("/api/net/{slot}/config")).await;

        assert_eq!(status, 200, "{config}");
        let read_only = strings_of(&config["read_only"]);
        for field in ["socks.port", "socks.enabled"] {
            assert!(
                read_only.iter().any(|f| f == field),
                "{field} must be read-only in an added slot: {read_only:?}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_main_slot_keeps_its_process_fields_editable() {
        let demo = setup(1).await;
        demo.add_slot().await;

        let (_, config) = demo.get("/api/config").await;

        let read_only = strings_of(&config["read_only"]);
        assert!(
            !read_only.iter().any(|f| f == "socks.port"),
            "socks.port must stay editable on the main slot: {read_only:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn saving_settings_is_busy() {
        let demo = setup(1).await;
        let slot = demo.add_slot().await;
        let path = format!("/api/net/{slot}/config");
        let (_, config) = demo.get(&path).await;
        let put = json!({"rev": config["rev"], "config": {"exit": {"enabled": true}}});

        let (status, body) = demo.put(&path, put).await;

        assert_eq!(status, 409, "{body}");
        assert_eq!(body["error"], "busy");
    }
}

mod leaving_a_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn the_second_slot_disappears_from_the_summary() {
        let demo = setup_with(2, &["--second-admin"]).await;
        let second = demo.slot_ids()[1].clone();

        let (status, body) = demo.leave(&second).await;

        assert_eq!(status, 200, "{body}");
        assert_eq!(
            ids_of(&body["nets"]),
            ["main"],
            "reply lists the networks left"
        );
        let (_, listed) = demo.get("/api/nets").await;
        assert_eq!(ids_of(&listed["nets"]), ["main"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_host_forgets_the_slot() {
        let demo = setup_with(2, &["--second-admin"]).await;
        let second = demo.slot_ids()[1].clone();

        demo.leave(&second).await;

        assert_eq!(demo.slot_ids(), ["main"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_main_slot_is_untouched() {
        let demo = setup(2).await;
        let second = demo.slot_ids()[1].clone();

        demo.leave(&second).await;

        let (status, _) = demo.get("/api/members").await;
        assert_eq!(status, 200);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_added_slot_can_be_left_too() {
        let demo = setup(1).await;
        let slot = demo.add_slot().await;

        let (status, body) = demo.leave(&slot).await;

        assert_eq!(status, 200, "{body}");
        assert_eq!(demo.slot_ids(), ["main"]);
    }
}
