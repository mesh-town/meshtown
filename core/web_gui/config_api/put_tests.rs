//! Tests for the switch choice `put_keeping` returns next to the response: which network the local
//! proxy must be moved to after a successful write, and no choice at all when the write was refused.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::config::config_file::{
    self, CliOverrides, ConfigSource, LoadedSettings, MainFile, PathRule, SaveError, Settings,
    SettingsFile,
};
use crate::transport::net::NodeSecret;
use crate::web_gui::{ActionError, LogPage, NodeControl, NodeSnapshot};

use super::super::put_write::ProxyPick;
use super::super::state::ConfigState;

const OTHER_NET: &str = "ab2c3d";

#[path = "put_choice_tests.rs"]
mod choice;

mod exits_choice {
    use super::*;

    mod from_empty_to_some {
        use super::*;

        #[test]
        fn picks_this_slot() {
            let mut fx = setup(Opts::default());
            let (status, pick) = fx.when_put(json!({"route": {"exits": [exit(1)]}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::ThisSlot);
        }
    }

    mod from_some_to_empty {
        use super::*;

        #[test]
        fn picks_nothing() {
            let mut fx = setup(Opts {
                exits: vec![exit(1)],
                ..Opts::default()
            });
            let (status, pick) = fx.when_put(json!({"route": {"exits": []}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                pick,
                Pick::None,
                "an empty exit list leaves nothing to choose"
            );
        }
    }

    mod from_one_exit_to_another {
        use super::*;

        #[test]
        fn picks_this_slot() {
            let mut fx = setup(Opts {
                exits: vec![exit(1)],
                ..Opts::default()
            });
            let (status, pick) = fx.when_put(json!({"route": {"exits": [exit(2)]}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::ThisSlot);
        }
    }

    mod sent_unchanged {
        use super::*;

        #[test]
        fn picks_nothing() {
            let mut fx = setup(Opts {
                exits: vec![exit(1)],
                ..Opts::default()
            });
            let (status, pick) = fx.when_put(json!({"route": {"exits": [exit(1)]}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::None, "the same list is not a change");
        }
    }
}

// --- setup and helpers ---

/// What the settings file holds before the PUT under test.
#[derive(Default)]
struct Opts {
    exits: Vec<String>,
    proxy_net: Option<&'static str>,
}

/// The choice, in a form that compares and prints.
#[derive(Debug, PartialEq)]
enum Pick {
    None,
    ThisSlot,
    Named(String),
}

struct Fixture {
    dir: PathBuf,
    state: ConfigState,
    node: RecordingNode,
}

fn setup(opts: Opts) -> Fixture {
    let dir = temp_dir();
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut fx = Fixture {
        dir,
        state: ConfigState::from_run(&run),
        node: RecordingNode::default(),
    };
    fx.write_initial(&opts);
    fx
}

impl Fixture {
    fn write_initial(&mut self, opts: &Opts) {
        let mut config = json!({});
        if !opts.exits.is_empty() {
            config["route"] = json!({"exits": opts.exits});
        }
        if let Some(net) = opts.proxy_net {
            config["proxy"] = json!({"net": net});
        }
        let (status, _) = self.when_put(config);
        assert_eq!(
            status,
            StatusCode::OK,
            "the initial file could not be written"
        );
    }

    fn when_put(&mut self, config: Value) -> (StatusCode, Pick) {
        let rev = self.state.file_report.rev;
        self.when_put_at(rev, config)
    }

    fn when_put_at(&mut self, rev: u64, config: Value) -> (StatusCode, Pick) {
        let body = json!({"rev": rev, "config": config})
            .to_string()
            .into_bytes();
        let (resp, pick) = self.state.put_keeping(&body, &self.node, None, &[]);
        (resp.status(), seen(pick))
    }

    /// The next save through the state's file is preceded by a write of someone else.
    fn race_with_a_foreign_write(&mut self) {
        let path = self.dir.join("config.json");
        self.state.file = Some(Arc::new(RacingFile::new(path)));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn seen(pick: Option<ProxyPick>) -> Pick {
    match pick {
        None => Pick::None,
        Some(ProxyPick::ThisSlot) => Pick::ThisSlot,
        Some(ProxyPick::Named(id)) => Pick::Named(id.to_string()),
    }
}

/// A peer id text for exit number `n`.
fn exit(n: u8) -> String {
    NodeSecret::from_bytes(&[n; 32]).peer_id().to_string()
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-put-pick-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A settings file that lets someone else save once right before the first save through it.
struct RacingFile {
    inner: MainFile,
    armed: Mutex<bool>,
}

impl RacingFile {
    fn new(path: PathBuf) -> Self {
        RacingFile {
            inner: MainFile(path),
            armed: Mutex::new(true),
        }
    }

    fn foreign_write(&self, now_unix: u64, secret: &NodeSecret, base: &Settings) {
        let LoadedSettings::Parsed { rev, settings } = self.inner.load(base) else {
            panic!("the file must be readable for the foreign write");
        };
        self.inner
            .save(Some(rev), now_unix, secret, &settings)
            .expect("the foreign write");
    }
}

impl SettingsFile for RacingFile {
    fn path(&self) -> &std::path::Path {
        self.inner.path()
    }
    fn reload(&self, prev: &Settings) -> Result<Settings, String> {
        self.inner.reload(prev)
    }
    fn load(&self, base: &Settings) -> LoadedSettings {
        self.inner.load(base)
    }
    fn save(
        &self,
        expected_rev: Option<u64>,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        if std::mem::replace(&mut *self.armed.lock().unwrap(), false) {
            self.foreign_write(now_unix, secret, s);
        }
        self.inner.save(expected_rev, now_unix, secret, s)
    }
    fn save_with_floor(
        &self,
        expected_rev: Option<u64>,
        rev_floor: u64,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        self.inner
            .save_with_floor(expected_rev, rev_floor, now_unix, secret, s)
    }
}

/// A node that only records the paths it was told to apply.
#[derive(Default)]
struct RecordingNode {
    applied: Mutex<Vec<Vec<String>>>,
}

impl NodeControl for RecordingNode {
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
        let mut applied = self.applied.lock().unwrap_or_else(|e| e.into_inner());
        applied.push(paths.iter().map(|p| p.to_string()).collect());
    }
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}
}
