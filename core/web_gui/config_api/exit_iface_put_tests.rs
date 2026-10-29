//! `PUT /api/config` and `GET /api/config` for `exit.interface`: applied live, written to the main
//! file or to a slot's own file, refused with the field's expectation, cleared by `null`. The harness is
//! from `mod_tests.rs` via `use super::*;`.

use super::*;
use crate::config::config_file::{
    load_net_file, save_net_file, LoadedNetFile, NetFile, NetSettings,
};

pub(super) const PATH: &str = "exit.interface";
const EXPECTED: &str =
    "an interface name (1–64 bytes, no control characters) or null — any network";

mod put_applies_live {
    use super::*;

    #[test]
    fn is_200_and_lists_the_path_as_changed() {
        // given
        let mut f = setup();
        // when
        let (status, v) = f.when_put(named("wg0"));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert_eq!(strings(&v["changed"]), [PATH]);
    }

    #[test]
    fn needs_no_restart() {
        // given
        let mut f = setup();
        // when
        let (_, v) = f.when_put(named("wg0"));
        // then
        assert!(
            !strings(&v["restart_fields"]).contains(&PATH.to_string()),
            "{v}"
        );
        assert!(
            !strings(&v["restart_pending"]).contains(&PATH.to_string()),
            "{v}"
        );
    }

    #[test]
    fn hands_the_path_to_the_node() {
        // given
        let mut f = setup();
        // when
        f.when_put(named("wg0"));
        // then: `apply_config` got exactly this path
        assert_eq!(f.applied(), vec![vec![PATH.to_string()]]);
        assert_eq!(
            f.state.file_settings.net.exit_interface.as_deref(),
            Some("wg0")
        );
    }

    #[test]
    fn writes_the_main_file() {
        // given
        let mut f = setup();
        // when
        f.when_put(named("wg0"));
        // then
        assert_eq!(f.main_file_value().as_deref(), Some("wg0"));
    }

    #[test]
    fn the_same_value_again_changes_nothing() {
        // given: the value is already saved
        let mut f = setup();
        f.when_put(named("wg0"));
        // when
        let (status, v) = f.when_put(named("wg0"));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert!(strings(&v["changed"]).is_empty(), "{v}");
    }
}

mod put_on_slot_writes_net_file {
    use super::*;

    #[test]
    fn the_slot_file_holds_the_name() {
        // given: a state over the file of an added network
        let mut f = setup_slot();
        // when
        let (status, v) = f.when_put(named("wg0"));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert_eq!(f.slot_file_value().as_deref(), Some("wg0"));
        assert_eq!(strings(&v["changed"]), [PATH]);
    }

    #[test]
    fn the_main_file_is_not_touched() {
        // given
        let mut f = setup_slot();
        let before = f.main_bytes();
        // when
        f.when_put(named("wg0"));
        // then
        assert_eq!(f.main_bytes(), before, "the main settings file");
    }

    #[test]
    fn the_node_is_told_the_path() {
        // given
        let mut f = setup_slot();
        // when
        f.when_put(named("wg0"));
        // then
        assert_eq!(f.applied(), vec![vec![PATH.to_string()]]);
    }
}

mod put_rejects_bad_name_422 {
    use super::*;

    fn refused(name: &str) {
        // given: a saved good value
        let mut f = setup();
        f.when_put(named("wg0"));
        let before = f.main_bytes();
        // when
        let (status, v) = f.when_put(named(name));
        // then: a field error with the expectation, and nothing written
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {v}");
        assert_eq!(v["error"], "invalid");
        assert_eq!(v["fields"][0]["path"], PATH);
        assert_eq!(v["fields"][0]["message"], EXPECTED);
        assert_eq!(f.main_bytes(), before, "the settings file");
        assert_eq!(
            f.applied().len(),
            1,
            "only the first, good PUT reached the node"
        );
    }

    #[test]
    fn empty() {
        refused("");
    }

    #[test]
    fn sixty_five_bytes() {
        refused(&"a".repeat(65));
    }

    #[test]
    fn spaces_around() {
        refused(" wg0 ");
    }

    #[test]
    fn a_number() {
        // given
        let mut f = setup();
        // when
        let (status, v) = f.when_put(serde_json::json!({"exit": {"interface": 5}}));
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {v}");
        assert_eq!(v["fields"][0]["path"], PATH);
    }
}

/// One settings file in a fresh directory, and a node that records what it was told to apply.
pub(super) struct Fixture {
    dir: PathBuf,
    pub(super) state: ConfigState,
    node: StubNode,
}

impl Fixture {
    /// A `PUT` of `config` at the current revision.
    pub(super) fn when_put(&mut self, config: Value) -> (StatusCode, Value) {
        let body = put_body(self.state.file_report.rev, config);
        body_json(self.state.put(&body, &self.node))
    }

    pub(super) fn applied(&self) -> Vec<Vec<String>> {
        self.node
            .applied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(super) fn main_text(&self) -> String {
        std::fs::read_to_string(self.dir.join("config.json")).unwrap_or_default()
    }

    pub(super) fn main_bytes(&self) -> Vec<u8> {
        std::fs::read(self.dir.join("config.json")).unwrap_or_default()
    }

    /// `exit.interface` as the main file holds it.
    pub(super) fn main_file_value(&self) -> Option<String> {
        match config_file::load_config_file(&self.dir.join("config.json")) {
            LoadedConfigFile::Parsed(p) => p.settings.net.exit_interface,
            other => panic!("expected a parsed main file, got {other:?}"),
        }
    }

    /// `exit.interface` as the slot's file holds it.
    pub(super) fn slot_file_value(&self) -> Option<String> {
        match load_net_file(&self.slot_path()) {
            LoadedNetFile::Parsed(p) => p.net.exit_interface,
            other => panic!("expected a parsed slot file, got {other:?}"),
        }
    }

    pub(super) fn slot_path(&self) -> PathBuf {
        self.dir.join("nets").join("ab2c3d").join("config.json")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        cleanup(&self.dir);
    }
}

pub(super) fn setup() -> Fixture {
    let dir = temp_dir();
    let state = new_state(&dir);
    Fixture {
        dir,
        state,
        node: StubNode::new(),
    }
}

/// Like `setup`, but the state reads and writes the file of an added network.
pub(super) fn setup_slot() -> Fixture {
    let mut f = setup();
    let path = f.slot_path();
    let rev = save_net_file(
        &path,
        None,
        1_790_000_000,
        5,
        &f.state.secret,
        &NetSettings::default(),
    )
    .expect("write the slot file");
    f.state.file = Some(std::sync::Arc::new(NetFile {
        path,
        created_unix: 5,
    }));
    f.state.file_report.rev = rev;
    f
}

pub(super) fn named(name: &str) -> Value {
    serde_json::json!({"exit": {"interface": name}})
}

pub(super) fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
