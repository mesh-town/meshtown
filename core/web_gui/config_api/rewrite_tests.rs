//! Tests for the writes the page makes to a slot's settings file on its own initiative:
//! `ConfigState::rewrite_settings` and `ConfigState::settle_socks_baseline`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::config::config_file::{
    self, CliOverrides, ConfigSource, IfaceFilter, ListenAccess, LoadedSettings, MainFile,
    PathRule, SaveError, Settings, SettingsFile,
};
use crate::transport::net::{NodeSecret, PeerId};
use crate::web_gui::{ActionError, LogPage, NodeControl, NodeSnapshot};

use super::super::state::ConfigState;

mod edit_that_changes_the_file {
    use super::*;

    #[test]
    fn reports_true_and_raises_the_revision_by_one() {
        let mut fx = setup(Opts::with_exits(&[]));
        let rev_before = fx.state.file_report.rev;
        let result = fx.when_exits_set(&[exit(1)]);
        assert_eq!(result, Ok(true));
        assert_eq!(fx.file_rev(), rev_before + 1, "revision on disk");
        assert_eq!(
            fx.state.file_report.rev,
            rev_before + 1,
            "revision in the state"
        );
    }

    #[test]
    fn the_file_holds_the_edit() {
        let mut fx = setup(Opts::with_exits(&[]));
        let result = fx.when_exits_set(&[exit(1), exit(2)]);
        assert_eq!(result, Ok(true));
        assert_eq!(fx.file_settings().net.route_exits, vec![exit(1), exit(2)]);
        assert_eq!(
            fx.state.file_settings.net.route_exits,
            vec![exit(1), exit(2)],
            "the state follows the file"
        );
    }

    #[test]
    fn leaves_the_rest_of_the_file_as_it_was() {
        let mut fx = setup(Opts::with_exits(&[exit(1)]));
        let before = fx.file_settings();
        let result = fx.when_exits_set(&[]);
        assert_eq!(result, Ok(true));
        let mut expected = before;
        expected.net.route_exits.clear();
        assert_eq!(fx.file_settings(), expected);
    }

    #[test]
    fn tells_the_node_to_apply_route_exits() {
        let mut fx = setup(Opts::with_exits(&[exit(1)]));
        let result = fx.when_exits_set(&[]);
        assert_eq!(result, Ok(true));
        assert_eq!(fx.node.applied(), vec![vec!["route.exits".to_string()]]);
    }
}

mod edit_that_changes_nothing {
    use super::*;

    #[test]
    fn reports_false() {
        let mut fx = setup(Opts::with_exits(&[]));
        assert_eq!(fx.when_exits_set(&[]), Ok(false));
    }

    #[test]
    fn leaves_the_file_byte_for_byte() {
        let mut fx = setup(Opts::with_exits(&[exit(1)]));
        let before = fx.file_bytes();
        let result = fx.when_exits_set(&[exit(1)]);
        assert_eq!(result, Ok(false));
        assert!(fx.file_bytes() == before, "the file was rewritten");
    }

    #[test]
    fn keeps_the_revision() {
        let mut fx = setup(Opts::with_exits(&[]));
        let rev_before = fx.state.file_report.rev;
        let _ = fx.when_exits_set(&[]);
        assert_eq!(fx.file_rev(), rev_before);
        assert_eq!(fx.state.file_report.rev, rev_before);
    }

    #[test]
    fn tells_the_node_nothing() {
        let mut fx = setup(Opts::with_exits(&[]));
        let _ = fx.when_exits_set(&[]);
        assert!(fx.node.applied().is_empty(), "{:?}", fx.node.applied());
    }
}

mod slot_without_a_file {
    use super::*;

    #[test]
    fn state_not_using_a_file_is_an_error() {
        let mut fx = setup(Opts::with_exits(&[]));
        fx.state.file = None;
        let result = fx.when_exits_set(&[exit(1)]);
        assert!(result.is_err(), "{result:?}");
        assert!(fx.node.applied().is_empty());
    }

    #[test]
    fn file_gone_from_disk_is_an_error() {
        let mut fx = setup(Opts::with_exits(&[]));
        std::fs::remove_file(fx.path()).unwrap();
        let result = fx.when_exits_set(&[exit(1)]);
        assert!(result.is_err(), "{result:?}");
        assert!(!fx.path().exists(), "the write must not create the file");
        assert!(fx.node.applied().is_empty());
    }
}

mod foreign_write_between_the_read_and_the_write {
    use super::*;

    #[test]
    fn is_an_error() {
        let mut fx = setup(Opts::with_exits(&[]));
        fx.race_with_a_foreign_write();
        let result = fx.when_exits_set(&[exit(1)]);
        assert!(
            result.is_err(),
            "a stale revision must not be overwritten: {result:?}"
        );
    }

    #[test]
    fn the_other_writers_file_stays_without_the_edit() {
        let mut fx = setup(Opts::with_exits(&[]));
        let rev_before = fx.file_rev();
        fx.race_with_a_foreign_write();
        let _ = fx.when_exits_set(&[exit(1)]);
        assert_eq!(
            fx.file_rev(),
            rev_before + 1,
            "only the foreign write landed"
        );
        assert!(fx.file_settings().net.route_exits.is_empty());
    }

    #[test]
    fn tells_the_node_nothing() {
        let mut fx = setup(Opts::with_exits(&[]));
        fx.race_with_a_foreign_write();
        let _ = fx.when_exits_set(&[exit(1)]);
        assert!(fx.node.applied().is_empty(), "{:?}", fx.node.applied());
    }
}

mod settle_socks_baseline {
    use super::*;

    mod everything_in_the_file_was_applied {
        use super::*;

        #[test]
        fn the_boot_baseline_takes_the_applied_values() {
            let mut fx = setup(Opts::default());
            fx.file_has_socks(false, 1090, ListenAccess::Localnet, IfaceFilter::Any);
            let applied = fx.state.file_settings.process.clone();
            fx.state.settle_socks_baseline(&applied);
            let boot = &fx.state.boot_file_settings.process;
            assert!(!boot.socks_enabled);
            assert_eq!(boot.socks_port, 1090);
            assert_eq!(boot.socks_access, ListenAccess::Localnet);
            assert_eq!(boot.socks_ifaces, IfaceFilter::Any);
        }

        #[test]
        fn no_socks_path_waits_for_a_restart() {
            let mut fx = setup(Opts::default());
            fx.file_has_socks(false, 1090, ListenAccess::Localnet, IfaceFilter::Any);
            assert_eq!(
                fx.pending(),
                vec![
                    "socks.enabled",
                    "socks.port",
                    "socks.access",
                    "socks.ifaces"
                ],
                "precondition: the file differs from the boot baseline"
            );
            let applied = fx.state.file_settings.process.clone();
            fx.state.settle_socks_baseline(&applied);
            assert_eq!(fx.pending(), Vec::<&str>::new());
        }

        #[test]
        fn the_file_settings_are_not_touched() {
            let mut fx = setup(Opts::default());
            fx.file_has_socks(true, 1090, ListenAccess::Localhost, IfaceFilter::Auto);
            let before = fx.state.file_settings.clone();
            let applied = before.process.clone();
            fx.state.settle_socks_baseline(&applied);
            assert_eq!(fx.state.file_settings, before);
        }
    }

    mod another_socks_field_changed_in_the_file_after_applying {
        use super::*;

        #[test]
        fn only_that_field_stays_pending() {
            // given: the file now has port 1090 (applied) and access localnet (not applied)
            let mut fx = setup(Opts::default());
            let mut applied = fx.state.file_settings.process.clone();
            applied.socks_port = 1090;
            fx.file_has_socks(true, 1090, ListenAccess::Localnet, IfaceFilter::Auto);
            // when
            fx.state.settle_socks_baseline(&applied);
            // then
            assert_eq!(fx.pending(), vec!["socks.access"]);
        }
    }

    mod same_field_changed_again_after_applying {
        use super::*;

        #[test]
        fn the_newer_file_value_stays_pending() {
            // given: port 1090 was applied, then the file moved on to 1100
            let mut fx = setup(Opts::default());
            let mut applied = fx.state.file_settings.process.clone();
            applied.socks_port = 1090;
            fx.file_has_socks(true, 1100, ListenAccess::Localhost, IfaceFilter::Auto);
            // when
            fx.state.settle_socks_baseline(&applied);
            // then: the baseline is what was applied, not what the file has now
            assert_eq!(fx.state.boot_file_settings.process.socks_port, 1090);
            assert_eq!(fx.pending(), vec!["socks.port"]);
        }
    }

    mod other_restart_fields_pending {
        use super::*;

        #[test]
        fn stay_pending() {
            let mut fx = setup(Opts::default());
            fx.file_has_socks(true, 1090, ListenAccess::Localhost, IfaceFilter::Auto);
            fx.state.file_settings.net.doh_server = "9.9.9.9:443".parse().unwrap();
            let applied = fx.state.file_settings.process.clone();
            fx.state.settle_socks_baseline(&applied);
            assert_eq!(fx.pending(), vec!["checks.doh_server"]);
        }
    }
}

// --- setup and helpers ---

#[derive(Default)]
struct Opts {
    exits: Vec<PeerId>,
}

impl Opts {
    fn with_exits(exits: &[PeerId]) -> Opts {
        Opts {
            exits: exits.to_vec(),
        }
    }
}

struct Fixture {
    dir: PathBuf,
    state: ConfigState,
    node: RecordingNode,
}

/// A state over a fresh settings file that holds `opts.exits`.
fn setup(opts: Opts) -> Fixture {
    let dir = temp_dir();
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    if !opts.exits.is_empty() {
        save_exits(&mut state, &opts.exits);
    }
    Fixture {
        dir,
        state,
        node: RecordingNode::default(),
    }
}

/// Puts `exits` into the file the way an earlier page save would have.
fn save_exits(state: &mut ConfigState, exits: &[PeerId]) {
    let file = state.file.clone().expect("the state has a file");
    let mut settings = state.file_settings.clone();
    settings.net.route_exits = exits.to_vec();
    let rev = file
        .save(Some(state.file_report.rev), 0, &state.secret, &settings)
        .expect("the initial save");
    state.file_settings = settings;
    state.file_report.rev = rev;
}

impl Fixture {
    fn when_exits_set(&mut self, exits: &[PeerId]) -> Result<bool, String> {
        let exits = exits.to_vec();
        self.state
            .rewrite_settings(&self.node, |s| s.net.route_exits = exits)
    }

    fn path(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    fn file_bytes(&self) -> Vec<u8> {
        std::fs::read(self.path()).unwrap()
    }

    fn loaded(&self) -> (u64, Settings) {
        match MainFile(self.path()).load(&self.state.file_settings) {
            LoadedSettings::Parsed { rev, settings } => (rev, settings),
            other => panic!("the file must be readable: {other:?}"),
        }
    }

    fn file_rev(&self) -> u64 {
        self.loaded().0
    }

    fn file_settings(&self) -> Settings {
        self.loaded().1
    }

    /// The `socks.*` values the file holds now (the boot baseline keeps the defaults).
    fn file_has_socks(
        &mut self,
        enabled: bool,
        port: u16,
        access: ListenAccess,
        ifaces: IfaceFilter,
    ) {
        let process = &mut self.state.file_settings.process;
        process.socks_enabled = enabled;
        process.socks_port = port;
        process.socks_access = access;
        process.socks_ifaces = ifaces;
    }

    fn pending(&self) -> Vec<&'static str> {
        self.state.pending_restart_paths()
    }

    /// The next save through the state's file is preceded by a write of someone else.
    fn race_with_a_foreign_write(&mut self) {
        self.state.file = Some(Arc::new(RacingFile::new(self.path())));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The peer id of exit number `n`.
fn exit(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-rewrite-{}-{}", std::process::id(), n));
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
    fn path(&self) -> &Path {
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

impl RecordingNode {
    fn applied(&self) -> Vec<Vec<String>> {
        self.applied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
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
