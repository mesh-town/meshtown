//! `/api/config` of the primary slot and of an added slot: what a write touches and what a read shows.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::{self, CliOverrides, ConfigSource, ListenAccess, PathRule};
use crate::web_gui::test_host::{send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::{App, ConfigState, HostControl};

const ADDED: &str = "ab2c3d";
/// Well-formed, but no slot of the fixture has it.
const UNKNOWN: &str = "zzzzzz";

mod put_to_the_primary_slot {
    use super::*;

    mod naming_an_unknown_proxy_net {
        use super::*;

        #[tokio::test]
        async fn is_404_and_the_file_is_not_changed() {
            // given
            let f = setup();
            let before = f.file_text("main");
            // when
            let (status, body) = f.when_put("main", json!({"proxy": {"net": UNKNOWN}})).await;
            // then
            assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
            assert_eq!(body["error"], "unknown_net");
            assert_eq!(f.file_text("main"), before, "file of the primary slot");
        }
    }

    mod changing_web_access {
        use super::*;

        #[tokio::test]
        async fn updates_the_access_of_the_page() {
            // given: the page runs with the other mode than the one about to be written
            let f = setup();
            f.app.shared.set_access(ListenAccess::Localhost);
            // when
            let (status, body) = f.when_put("main", json!({"web": {"access": "localnet"}})).await;
            // then
            assert_eq!(status, StatusCode::OK, "body: {body}");
            assert_eq!(f.app.shared.auth.access(), ListenAccess::Localnet);
        }
    }
}

mod put_to_an_added_slot {
    use super::*;

    #[tokio::test]
    async fn naming_an_unknown_proxy_net_does_not_touch_the_primary_file() {
        // given
        let f = setup();
        let main_before = f.file_text("main");
        assert!(!main_before.is_empty(), "the primary file exists");
        // when
        let (status, body) = f.when_put(ADDED, json!({"proxy": {"net": UNKNOWN}})).await;
        // then: the check of `proxy.net` is the primary slot's; an added slot takes the write as it is
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(f.file_text("main"), main_before, "file of the primary slot");
    }

    #[tokio::test]
    async fn does_not_change_the_access_of_the_page() {
        // given: the page runs with the mode that differs from the file of the primary slot
        let f = setup();
        let kept = other_than(f.main_access());
        f.app.shared.set_access(kept);
        // when
        let (status, body) = f.when_put(ADDED, json!({"checks": {"interval_sec": 600}})).await;
        // then
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(f.app.shared.auth.access(), kept, "access of the page");
    }
}

mod get_of_an_added_slot {
    use super::*;

    #[tokio::test]
    async fn shows_the_socks_port_of_the_primary_file() {
        // given
        let f = setup();
        let (put, body) = f.when_put("main", json!({"socks": {"port": 4242}})).await;
        assert_eq!(put, StatusCode::OK, "body: {body}");
        // when
        let (status, body) = send(&f.router, Method::GET, &format!("/api/net/{ADDED}/config"), "").await;
        // then
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["config"]["socks"]["port"], 4242);
    }
}

fn other_than(mode: ListenAccess) -> ListenAccess {
    match mode {
        ListenAccess::Localhost => ListenAccess::Localnet,
        ListenAccess::Localnet => ListenAccess::Localhost,
    }
}

/// The primary slot and `ab2c3d`, both over settings files of their own, behind the real router.
struct Fixture {
    router: axum::Router,
    app: Arc<App>,
    host: Arc<FakeHost>,
    dir: PathBuf,
}

impl Fixture {
    /// A `PUT` of `config` to the slot at the current revision of its file.
    async fn when_put(&self, slot: &str, config: Value) -> (StatusCode, Value) {
        let path = match slot {
            "main" => "/api/config".to_string(),
            id => format!("/api/net/{id}/config"),
        };
        let body = json!({"rev": self.rev_of(slot), "config": config}).to_string();
        send(&self.router, Method::PUT, &path, &body).await
    }

    fn rev_of(&self, slot: &str) -> u64 {
        let handle = self.host.slot(&config_file::NetSlotId::parse(slot).unwrap()).unwrap();
        let rev = handle.config.lock().unwrap().file_report.rev;
        rev
    }

    fn file_text(&self, slot: &str) -> String {
        std::fs::read_to_string(self.dir.join(slot).join("config.json")).unwrap_or_default()
    }

    fn main_access(&self) -> ListenAccess {
        let main = self.host.main_slot();
        let access = main.config.lock().unwrap().file_settings.process.web_access;
        access
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup() -> Fixture {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("routes-config-{}-{n}", std::process::id()));
    let main = file_slot("main", &dir.join("main"), None);
    let added = file_slot(ADDED, &dir.join(ADDED), Some("some-network-key"));
    let host = FakeHost::new(vec![main, added]);
    let (router, app) = test_app(host.clone());
    Fixture { router, app, host, dir }
}

fn file_slot(id: &str, dir: &std::path::Path, key: Option<&str>) -> crate::web_gui::SlotHandle {
    std::fs::create_dir_all(dir).unwrap();
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    state.current_network_key = key.map(str::to_string);
    slot_handle(id, SnapNode::new(Default::default()), state)
}
