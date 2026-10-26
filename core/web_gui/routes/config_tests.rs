//! `/api/config` of the primary slot and of an added slot: what a write touches and what a read shows.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::{self, CliOverrides, ConfigSource, ListenAccess, PathRule};
use crate::members::network_key::test_net;
use crate::ports_sharing::{Opened, Proto};
use crate::transport::net::NodeSecret;
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
            let (status, body) = f
                .when_put("main", json!({"web": {"access": "localnet"}}))
                .await;
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
        let (status, body) = f
            .when_put(ADDED, json!({"checks": {"interval_sec": 600}}))
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(f.app.shared.auth.access(), kept, "access of the page");
    }
}

mod a_local_opened_in_another_network {
    use super::*;

    #[tokio::test]
    async fn refuses_the_put_to_an_added_slot_and_keeps_its_file() {
        // given: the primary slot has opened `local 9000`
        let f = setup();
        f.given_opened("main", 9000);
        let before = f.file_text(ADDED);
        // when
        let (status, body) = f.when_put(ADDED, opens(9000)).await;
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
        assert_eq!(body["fields"][0]["path"], "ports.opened");
        assert_eq!(
            body["fields"][0]["message"],
            "local port 9000 is already opened in network \"home\""
        );
        assert_eq!(f.file_text(ADDED), before, "file of the added slot");
    }

    #[tokio::test]
    async fn refuses_the_put_to_the_primary_slot_and_keeps_its_file() {
        // given: the added slot has opened `local 9000`
        let f = setup();
        f.given_opened(ADDED, 9000);
        let before = f.file_text("main");
        // when
        let (status, body) = f.when_put("main", opens(9000)).await;
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
        assert_eq!(body["fields"][0]["path"], "ports.opened");
        assert_eq!(
            body["fields"][0]["message"],
            "local port 9000 is already opened in network \"work\""
        );
        assert_eq!(f.file_text("main"), before, "file of the primary slot");
    }

    #[tokio::test]
    async fn lets_another_local_through() {
        // given
        let f = setup();
        f.given_opened("main", 9000);
        // when
        let (status, body) = f.when_put(ADDED, opens(9001)).await;
        // then
        assert_eq!(status, StatusCode::OK, "body: {body}");
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
        let (status, body) = send(
            &f.router,
            Method::GET,
            &format!("/api/net/{ADDED}/config"),
            "",
        )
        .await;
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
        let handle = self
            .host
            .slot(&config_file::NetSlotId::parse(slot).unwrap())
            .unwrap();
        let rev = handle.config.lock().unwrap().file_report.rev;
        rev
    }

    fn file_text(&self, slot: &str) -> String {
        std::fs::read_to_string(self.dir.join(slot).join("config.json")).unwrap_or_default()
    }

    /// The settings of the slot hold one opened port with this `local`.
    fn given_opened(&self, slot: &str, local: u16) {
        let handle = self
            .host
            .slot(&config_file::NetSlotId::parse(slot).unwrap())
            .unwrap();
        let node = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
        let opened = Opened {
            proto: Proto::Tcp,
            node,
            port: 80,
            local,
        };
        handle.config.lock().unwrap().file_settings.net.ports_opened = vec![opened];
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

/// A config part that opens a port of a peer on `local`.
fn opens(local: u16) -> Value {
    let node = NodeSecret::from_bytes(&[7u8; 32]).peer_id().to_string();
    json!({"ports": {"opened": [{"node": node, "port": 80, "local": local}]}})
}

fn labelled_key(label: &str) -> String {
    test_net(label).text
}

fn setup() -> Fixture {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("routes-config-{}-{n}", std::process::id()));
    let main = file_slot("main", &dir.join("main"), Some(&labelled_key("home")));
    let added = file_slot(ADDED, &dir.join(ADDED), Some(&labelled_key("work")));
    let host = FakeHost::new(vec![main, added]);
    let (router, app) = test_app(host.clone());
    Fixture {
        router,
        app,
        host,
        dir,
    }
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
