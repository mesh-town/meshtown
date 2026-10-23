//! The demo's slots seen through the real page: `--nets N`, adding a slot, leaving one.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use meshtown_core::web_gui::{self, WebHandle, WebOptions};

use super::DemoHost;
use crate::args::Args;
use crate::config::prepare_demo_config;
use crate::shutdown::cleanup_dir;

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
        let demo = setup(2).await;
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
        let demo = setup(2).await;
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

// ============================================================================================
// Setup and helpers
// ============================================================================================

const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";

/// The demo prepares one settings directory per process, so the page tests run one at a time.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A demo host behind the real page on a free loopback port.
struct Demo {
    host: Arc<DemoHost>,
    handle: WebHandle,
    dir: Option<PathBuf>,
    _turn: tokio::sync::MutexGuard<'static, ()>,
}

/// `--nets <nets>`; the page starts on port 0 with the fixed token.
async fn setup(nets: u8) -> Demo {
    let turn = ONE_AT_A_TIME.lock().await;
    let flags = [
        "web_demo",
        "--nets",
        &nets.to_string(),
        "--port",
        "0",
        "--token",
        TOKEN,
    ];
    let args = Args::parse_from(flags);
    let (run, dir) = prepare_demo_config(&args, 1_700_000_000).expect("demo config");
    let main_node = Arc::new(crate::build_demo_node(&args, &run));
    let host = DemoHost::new(&args, &run, main_node);
    let mut opts = WebOptions::for_run(&run);
    opts.port = args.port;
    opts.fixed_token = args.token.clone();
    let handle = web_gui::start_web_page(opts, &run, host.clone())
        .await
        .unwrap_or_else(|e| panic!("the page did not start: {e}"));
    Demo {
        host,
        handle,
        dir,
        _turn: turn,
    }
}

impl Drop for Demo {
    fn drop(&mut self) {
        let _ = self.handle.stop.send(true);
        self.handle.task.abort();
        if let Some(dir) = &self.dir {
            cleanup_dir(dir);
        }
    }
}

impl Demo {
    fn slot_ids(&self) -> Vec<String> {
        use meshtown_core::web_gui::HostControl;
        let slots = self.host.slots();
        slots.iter().map(|s| s.id.as_str().to_string()).collect()
    }

    async fn get(&self, path: &str) -> (u16, Value) {
        self.send("GET", path, None).await
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("POST", path, Some(body)).await
    }

    async fn put(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("PUT", path, Some(body)).await
    }

    /// `POST /api/nets`, returning the id of the slot the page created.
    async fn add_slot(&self) -> String {
        let (status, body) = self.post("/api/nets", json!({})).await;
        assert_eq!(status, 201, "{body}");
        body["slot"].as_str().expect("slot id").to_string()
    }

    async fn leave(&self, slot: &str) -> (u16, Value) {
        let path = format!("/api/net/{slot}/members/leave");
        self.post(&path, json!({"confirm": true})).await
    }

    /// One HTTP/1.1 request on a fresh connection; the reply body is read as JSON (`Null` if it is not).
    async fn send(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let port = self.handle.shared.port();
        let payload = body.map(|b| b.to_string()).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nx-meshtown-token: {TOKEN}\r\nOrigin: http://127.0.0.1:{port}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect to the page");
        stream.write_all(request.as_bytes()).await.expect("send");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.expect("read reply");
        parse_reply(&String::from_utf8_lossy(&raw))
    }
}

fn parse_reply(raw: &str) -> (u16, Value) {
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or_default();
    let json = match (body.find('{'), body.rfind('}')) {
        (Some(from), Some(to)) if from < to => serde_json::from_str(&body[from..=to]),
        _ => Ok(Value::Null),
    };
    (status, json.unwrap_or(Value::Null))
}

fn ids_of(nets: &Value) -> Vec<String> {
    let list = nets.as_array().expect("a nets array");
    list.iter()
        .map(|n| n["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn strings_of(list: &Value) -> Vec<String> {
    let items = list.as_array().expect("a list of strings");
    items
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// `main`, or six characters of `[a-z2-7]`.
fn is_slot_id(id: &str) -> bool {
    id == "main" || (id.len() == 6 && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7')))
}
