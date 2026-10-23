//! Restart of a slot that has no network: only the primary slot may restart the process-wide proxy.

use std::net::TcpListener;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::ProcessSettings;
use crate::proxy::ProxyFront;
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost};
use crate::web_gui::{ActionError, LogPage, NodeControl, NodeSnapshot};

const NO_KEY: &str = "no network key yet";

mod primary_slot_without_a_key {
    use super::*;

    mod socks_port_changed_in_the_file {
        use super::*;

        #[tokio::test]
        async fn restart_is_accepted() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                ..Opts::default()
            })
            .await;
            // when
            let (status, body) = f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
            assert_eq!(body, json!({"restart": true}));
        }

        #[tokio::test]
        async fn proxy_listens_on_the_new_port() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                ..Opts::default()
            })
            .await;
            // when
            f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(f.front_port(), Some(f.new_port), "port of the front");
            assert!(can_connect(f.new_port), "the new port accepts connections");
        }

        #[tokio::test]
        async fn restart_is_no_longer_pending() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                ..Opts::default()
            })
            .await;
            assert_eq!(f.restart_slots().await, json!(["main"]), "pending before");
            // when
            f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(f.restart_slots().await, json!([]), "pending after");
        }
    }

    mod socks_settings_unchanged {
        use super::*;

        #[tokio::test]
        async fn restart_is_refused_as_busy_with_the_node_text() {
            // given
            let f = setup(Opts::default()).await;
            // when
            let (status, body) = f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(body["error"], "busy");
            assert_eq!(body["message"], NO_KEY);
        }

        #[tokio::test]
        async fn proxy_stays_on_its_port() {
            // given
            let f = setup(Opts::default()).await;
            // when
            f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(f.front_port(), Some(f.old_port));
        }
    }

    mod socks_port_pinned_by_a_flag {
        use super::*;

        #[tokio::test]
        async fn restart_is_refused_and_the_proxy_stays() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                port_pinned_by_flag: true,
                ..Opts::default()
            })
            .await;
            // when
            let (status, body) = f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
            assert_eq!(body["error"], "busy");
            assert_eq!(f.front_port(), Some(f.old_port), "port of the front");
        }
    }

    mod new_port_held_by_another_program {
        use super::*;

        #[tokio::test]
        async fn restart_is_refused_with_the_bind_error_text() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                new_port_held: true,
                ..Opts::default()
            })
            .await;
            // when
            let (status, body) = f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
            assert_eq!(body["error"], "busy");
            let expected = format!(
                "port {} for the browser proxy is in use by another program",
                f.new_port
            );
            let message = body["message"].as_str().unwrap_or_default();
            assert!(message.contains(&expected), "message: {message}");
        }

        #[tokio::test]
        async fn old_port_keeps_listening_and_the_restart_stays_pending() {
            // given
            let f = setup(Opts {
                socks_port_changed: true,
                new_port_held: true,
                ..Opts::default()
            })
            .await;
            // when
            f.when_restart("/api/actions/restart").await;
            // then
            assert_eq!(f.front_port(), Some(f.old_port), "port of the front");
            assert!(can_connect(f.old_port), "the old port accepts connections");
            assert_eq!(f.restart_slots().await, json!(["main"]), "still pending");
        }
    }
}

mod added_slot_without_a_key {
    use super::*;

    #[tokio::test]
    async fn restart_is_refused_though_socks_settings_wait() {
        // given
        let f = setup(Opts {
            added_slot_socks_port_changed: true,
            ..Opts::default()
        })
        .await;
        // when
        let (status, body) = f.when_restart("/api/net/ab2c3d/actions/restart").await;
        // then
        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["error"], "busy");
        assert_eq!(f.front_port(), Some(f.old_port), "the proxy is not touched");
    }
}

/// Preconditions of a scenario; the default is a settled node with nothing waiting.
#[derive(Default)]
struct Opts {
    /// `socks.port` in the file differs from the one the process runs on.
    socks_port_changed: bool,
    /// The same for the added slot `ab2c3d`.
    added_slot_socks_port_changed: bool,
    /// The port comes from a launch flag.
    port_pinned_by_flag: bool,
    /// Another program holds the new port.
    new_port_held: bool,
}

struct Fixture {
    router: axum::Router,
    front: ProxyFront,
    old_port: u16,
    new_port: u16,
    _held: Option<TcpListener>,
}

impl Fixture {
    async fn when_restart(&self, path: &str) -> (StatusCode, Value) {
        send(&self.router, Method::POST, path, "").await
    }

    fn front_port(&self) -> Option<u16> {
        self.front.addr().map(|a| a.port())
    }

    /// The slots whose settings wait for a restart, as the overview lists them.
    async fn restart_slots(&self) -> Value {
        let (_, body) = send(&self.router, Method::GET, "/api/overview", "").await;
        body["restart_slots"].clone()
    }
}

async fn setup(opts: Opts) -> Fixture {
    let old_port = free_port();
    let new_port = free_port();
    let held = opts
        .new_port_held
        .then(|| TcpListener::bind(("127.0.0.1", new_port)).expect("hold the new port"));
    let front = ProxyFront::bind(&ProcessSettings {
        socks_port: old_port,
        ..ProcessSettings::default()
    })
    .await
    .expect("the front binds a free port");
    let ports = (old_port, new_port);
    let main = keyless_slot("main", opts.socks_port_changed, ports);
    if opts.port_pinned_by_flag {
        main.config.lock().unwrap().cli_override_paths = vec!["socks.port"];
    }
    let added = keyless_slot("ab2c3d", opts.added_slot_socks_port_changed, ports);
    let host = FakeHost::new(vec![main, added]);
    host.set_front(Some(front.clone()));
    let router = test_app(host).0;
    Fixture {
        router,
        front,
        old_port,
        new_port,
        _held: held,
    }
}

/// A slot with no network key whose file may already name the new port.
fn keyless_slot(id: &str, changed: bool, ports: (u16, u16)) -> crate::web_gui::SlotHandle {
    let (old_port, new_port) = ports;
    let mut config = config_with_key(None);
    config.file_settings.process.socks_port = old_port;
    config.boot_file_settings.process.socks_port = old_port;
    if changed {
        config.file_settings.process.socks_port = new_port;
    }
    slot_handle(id, Arc::new(KeylessNode), config)
}

fn free_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    probe.local_addr().unwrap().port()
}

fn can_connect(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// A node that has no network yet: it refuses to restart.
struct KeylessNode;

impl NodeControl for KeylessNode {
    fn snapshot(&self) -> NodeSnapshot {
        NodeSnapshot::default()
    }
    fn log_after(&self, _after: u64, _limit: usize, _net: Option<&str>) -> LogPage {
        LogPage::default()
    }
    fn find_exit(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_domain(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_all(&self) -> Result<u64, ActionError> {
        Ok(0)
    }
    fn add_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn remove_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn add_peer_address(&self, _line: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_config(
        &self,
        _settings: &crate::config::config_file::Settings,
        _paths: &[&'static str],
    ) {
    }
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}
    fn restart(&self) -> Result<(), ActionError> {
        Err(ActionError::NoNetwork(NO_KEY.to_string()))
    }
}
