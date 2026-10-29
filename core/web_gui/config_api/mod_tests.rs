//! Tests for `config_api`: `ConfigState::page_config_document`/`put`, `stale_outcome` — moved from the old
//! monolith unchanged, only the `use` lines were rebuilt for the split files.

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use axum::response::Response;
use serde_json::Value;

use crate::config::config_file::{
    self, CliOverrides, ConfigSource, LoadedConfigFile, LoadedSettings, PathRule, Settings,
};
use crate::web_gui::{ActionError, LogPage, NodeControl, NodeSnapshot};

use super::put_write::{stale_outcome, StaleOutcome};
use super::ConfigState;

/// A synchronous futures driver — this module never writes an await expression as text anywhere in the source (including tests):
/// a text grep over the file for 0 matches checks that.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
        .block_on(fut)
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mesh-config-api-{}-{}-{}",
        std::process::id(),
        nanos,
        n
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn new_state(dir: &Path) -> ConfigState {
    let path = dir.join("config.json");
    let source = ConfigSource::File {
        path,
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    ConfigState::from_run(&run)
}

fn put_body(rev: u64, config: Value) -> Vec<u8> {
    serde_json::json!({"rev": rev, "config": config})
        .to_string()
        .into_bytes()
}

fn body_json(resp: Response) -> (StatusCode, Value) {
    use http_body_util::BodyExt;
    let status = resp.status();
    let collected = resp.into_body().collect();
    let bytes = block_on(collected).unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    (status, v)
}

struct StubNode {
    applied: std::sync::Mutex<Vec<Vec<String>>>,
}

impl StubNode {
    fn new() -> Self {
        StubNode {
            applied: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl NodeControl for StubNode {
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
    fn apply_config(&self, _settings: &Settings, paths: &[&'static str]) {
        self.applied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(paths.iter().map(|s| s.to_string()).collect());
    }
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}
}

#[test]
fn document_without_key() {
    let dir = temp_dir();
    let state = new_state(&dir);
    let doc = state.page_config_document();
    assert_eq!(
        doc["network"],
        serde_json::json!({"label": "", "masked": ""})
    );
    // `prepare_absent` puts a random name into the file it creates — it goes straight into `GET
    // /api/config`, so here it is not `Null` but two words from the pool.
    let name = doc["node"]["name"]
        .as_str()
        .expect("node name in the document");
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2, "{name}");
    assert!(crate::members::words::is_tail_word(parts[0]), "{name}");
    assert!(crate::members::words::is_tail_word(parts[1]), "{name}");
    assert_eq!(doc["autostart"]["available"], Value::Bool(false));
    assert_eq!(
        doc["autostart"]["reason"],
        "only in the desktop app with the tray icon"
    );
    cleanup(&dir);
}

#[test]
fn document_with_key_masks_tail_and_hides_it() {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let tail = "finch-unwary-k7m2q9x4rb8vhn3tzd6w5";
    let key = format!("our-gaming-room--{tail}");
    state.current_network_key = Some(key);
    let doc = state.page_config_document();
    let expected_masked = format!("our-gaming-room--{}", "•".repeat(tail.chars().count()));
    assert_eq!(doc["network"]["masked"], Value::String(expected_masked));
    let dump = serde_json::to_string(&doc).unwrap();
    assert!(!dump.contains("k7m2q9"));
    cleanup(&dir);
}

#[test]
fn stale_outcome_on_invalid_is_failed() {
    let outcome = stale_outcome(LoadedSettings::Invalid("x".to_string()));
    assert!(matches!(outcome, StaleOutcome::Failed));
}

mod get_of_a_file {
    use super::*;

    mod without_the_proxy_section {
        use super::*;

        #[test]
        fn has_no_proxy_key_in_the_config() {
            // given: a file written before `proxy.net` existed
            let dir = temp_dir();
            let state = state_from_text(&dir, OLD_FILE);
            // when
            let body = state.config_get_body();
            cleanup(&dir);
            // then: the default is not written, so no empty section appears
            let config = body["config"].as_object().expect("config is an object");
            assert!(!config.contains_key("proxy"), "config: {config:?}");
            assert!(config.contains_key("socks"), "the other sections stay");
        }
    }

    mod with_a_proxy_net {
        use super::*;

        #[test]
        fn shows_the_chosen_network() {
            let dir = temp_dir();
            let state = state_from_text(&dir, &old_file_with(r#","proxy":{"net":"ab2c3d"}"#));
            let body = state.config_get_body();
            cleanup(&dir);
            assert_eq!(
                body["config"]["proxy"],
                serde_json::json!({"net": "ab2c3d"})
            );
        }
    }

    mod with_proxy_net_main {
        use super::*;

        #[test]
        fn has_no_proxy_key_either() {
            let dir = temp_dir();
            let state = state_from_text(&dir, &old_file_with(r#","proxy":{"net":"main"}"#));
            let body = state.config_get_body();
            cleanup(&dir);
            assert!(body["config"].get("proxy").is_none(), "{}", body["config"]);
        }
    }
}

const OLD_FILE: &str = r#"{"rev":3,"saved_at":"2023-11-14T22:13:20Z","identity":{"secret":"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="},"config":{"version":1}}"#;

/// `OLD_FILE` with extra `config` members (a leading comma and the members) appended.
fn old_file_with(extra: &str) -> String {
    OLD_FILE.replacen(r#""version":1"#, &format!(r#""version":1{extra}"#), 1)
}

fn state_from_text(dir: &Path, text: &str) -> ConfigState {
    let path = dir.join("config.json");
    std::fs::write(&path, text).unwrap();
    let source = ConfigSource::File {
        path,
        may_create: false,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    ConfigState::from_run(&run)
}

#[cfg(test)]
#[path = "mod_put_tests.rs"]
mod put_tests;

#[cfg(test)]
#[path = "mod_put_retired_tests.rs"]
mod put_retired_tests;

#[cfg(test)]
#[path = "mod_put_relay_tests.rs"]
mod put_relay_tests;

#[cfg(test)]
#[path = "ports_tests.rs"]
mod ports_tests;

#[cfg(test)]
#[path = "put_opened_tests.rs"]
mod put_opened_tests;

#[cfg(test)]
#[path = "put_golden_tests.rs"]
mod put_golden_tests;

#[cfg(test)]
#[path = "put_lockout_tests.rs"]
mod put_lockout_tests;

#[cfg(test)]
#[path = "put_reapply_tests.rs"]
mod put_reapply_tests;

#[cfg(test)]
#[path = "mod_autostart_tests.rs"]
mod autostart_tests;

#[cfg(test)]
#[path = "exit_iface_put_tests.rs"]
mod exit_iface_put_tests;

#[cfg(test)]
#[path = "exit_iface_get_tests.rs"]
mod exit_iface_get_tests;
