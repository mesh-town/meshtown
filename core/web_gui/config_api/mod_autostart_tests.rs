//! Start at login in `GET`/`PUT /api/config`: the page shows the login entry of the system, a save changes it
//! through the backend (a scripted fake). The harness (`temp_dir`, `new_state`, `StubNode`, …) is from `mod_tests.rs`.

use super::*;
use crate::autostart::fake::{leaked, leaked_with, FakeAutostart};
use crate::autostart::{AutostartBackend, EntryState};
use serde_json::json;

const NO_BACKEND_REASON: &str = "only in the desktop app with the tray icon";

mod get {
    use super::*;

    mod without_a_backend {
        use super::*;

        #[test]
        fn get_without_backend_is_unavailable() {
            let fx = setup(None);
            assert_eq!(fx.shown(), view(false, false, NO_BACKEND_REASON));
        }

        #[test]
        fn get_without_backend_hides_true_from_file() {
            // given: `true` was saved earlier and the node now starts without a backend
            let fx = setup_saved_on(None);
            assert!(fx.saved_flag(), "precondition: the file says true");
            // when / then
            assert_eq!(fx.shown(), view(false, false, NO_BACKEND_REASON));
        }
    }

    mod with_a_backend {
        use super::*;

        #[test]
        fn get_absent_entry_is_off() {
            assert_eq!(entry(EntryState::Absent).shown(), view(false, true, ""));
        }

        #[test]
        fn get_current_entry_is_on_over_file_false() {
            let fx = entry(EntryState::Current);
            assert!(!fx.saved_flag(), "precondition: the file says false");
            assert_eq!(fx.shown(), view(true, true, ""));
        }

        #[test]
        fn get_stale_entry_is_on() {
            assert_eq!(entry(EntryState::Stale).shown(), view(true, true, ""));
        }

        #[test]
        fn get_switched_off_entry_is_off() {
            // given: the file says true, the user switched the entry off in the system
            let fx = setup_saved_on(Some(leaked(EntryState::SwitchedOff)));
            assert!(fx.saved_flag(), "precondition: the file says true");
            // when / then: the system wins
            assert_eq!(fx.shown(), view(false, true, ""));
        }

        #[test]
        fn get_read_error_makes_unavailable() {
            let fx = setup(Some(leaked_with(Err("no access".into()), Ok(()), Ok(()))));
            let reason = "could not read the login entry: no access";
            assert_eq!(fx.shown(), view(false, false, reason));
        }
    }
}

mod put_without_a_backend {
    use super::*;

    #[test]
    fn put_true_without_backend_is_not_available() {
        let mut fx = setup(None);
        let before = fx.file_text();
        let (status, v) = fx.put(enable());
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(v["error"], "not_available", "{v}");
        assert_eq!(v["message"], NO_BACKEND_REASON, "{v}");
        assert_eq!(fx.file_text(), before, "the file is not touched");
    }

    #[test]
    fn put_false_without_backend_is_accepted() {
        let mut fx = setup(None);
        let v = fx.put_ok(json!({"autostart": {"enabled": false}}));
        assert_eq!(v["config"]["autostart"]["enabled"], false);
    }
}

mod put_true {
    use super::*;

    #[test]
    fn put_true_enables_absent_entry() {
        // given
        let mut fx = entry(EntryState::Absent);
        // when
        let v = fx.put_ok(enable());
        // then: the entry is created, the choice is saved, the answer shows the new state
        assert_eq!(fx.changes(), ["enable"]);
        assert!(fx.saved_flag(), "the file mirrors the choice");
        assert_eq!(v["config"]["autostart"]["enabled"], true);
        assert!(v["changed"]
            .as_array()
            .unwrap()
            .contains(&json!("autostart.enabled")));
    }

    #[test]
    fn put_true_keeps_current_entry() {
        let mut fx = entry(EntryState::Current);
        fx.put_ok(enable());
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_true_rewrites_stale_entry() {
        let mut fx = entry(EntryState::Stale);
        fx.put_ok(enable());
        assert_eq!(fx.changes(), ["enable"]);
    }

    #[test]
    fn put_true_enables_switched_off_entry() {
        let mut fx = entry(EntryState::SwitchedOff);
        fx.put_ok(enable());
        assert_eq!(fx.changes(), ["enable"]);
    }
}

mod put_false {
    use super::*;

    #[test]
    fn put_false_removes_current_entry() {
        // given: the file says true and the entry is there
        let mut fx = setup_saved_on(Some(leaked(EntryState::Current)));
        // when
        let v = fx.put_ok(disable());
        // then
        assert_eq!(fx.changes(), ["disable"]);
        assert!(!fx.saved_flag(), "the file mirrors the choice");
        assert_eq!(v["config"]["autostart"]["enabled"], false);
    }

    #[test]
    fn put_false_removes_stale_entry() {
        let mut fx = setup_saved_on(Some(leaked(EntryState::Stale)));
        fx.put_ok(disable());
        assert_eq!(fx.changes(), ["disable"]);
    }

    #[test]
    fn put_false_keeps_absent_entry() {
        let mut fx = entry(EntryState::Absent);
        fx.put_ok(disable());
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_false_leaves_switched_off_entry() {
        let mut fx = setup_saved_on(Some(leaked(EntryState::SwitchedOff)));
        fx.put_ok(disable());
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
        assert!(!fx.saved_flag(), "the file mirrors the choice");
    }
}

mod system_refuses {
    use super::*;

    #[test]
    fn put_true_os_error_is_422_file_untouched() {
        // given
        let fake = leaked_with(Ok(EntryState::Absent), Err("denied".into()), Ok(()));
        let mut fx = setup(Some(fake));
        let (before, rev) = (fx.file_text(), fx.state.file_report.rev);
        // when
        let (status, v) = fx.put(enable());
        // then
        let text = "could not turn on start at login: denied";
        assert_refused(status, &v, text);
        assert_eq!(fx.file_text(), before, "the file is not touched");
        assert_eq!(fx.state.file_report.rev, rev);
    }

    #[test]
    fn put_false_os_error_is_422() {
        let fake = leaked_with(Ok(EntryState::Current), Ok(()), Err("busy".into()));
        let mut fx = setup_saved_on(Some(fake));
        let before = fx.file_text();
        let (status, v) = fx.put(disable());
        let text = "could not turn off start at login: busy";
        assert_refused(status, &v, text);
        assert_eq!(fx.file_text(), before, "the file is not touched");
    }

    #[test]
    fn put_true_with_read_error_is_not_available() {
        let mut fx = setup(Some(leaked_with(Err("no access".into()), Ok(()), Ok(()))));
        let (status, v) = fx.put(enable());
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert_eq!(v["error"], "not_available", "{v}");
        assert_eq!(v["message"], "could not read the login entry: no access");
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_false_with_read_error_saves() {
        // given: the entry can't be read, the user saves another field
        let mut fx = setup(Some(leaked_with(Err("no access".into()), Ok(()), Ok(()))));
        // when
        let v = fx.put_ok(json!({"autostart": {"enabled": false}, "socks": {"port": 1090}}));
        // then: the save goes through and the entry is left alone
        assert_eq!(v["config"]["socks"]["port"], 1090);
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }
}

mod neighbouring_inputs {
    use super::*;

    #[test]
    fn put_other_field_leaves_entry_alone() {
        // given: the page sends the autostart section exactly as GET showed it
        let mut fx = entry(EntryState::Current);
        let shown = fx.shown();
        // when
        fx.put_ok(json!({"autostart": shown, "socks": {"port": 1090}}));
        // then
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_stale_rev_does_not_touch_entry() {
        // given: the file moved on after the page loaded
        let mut fx = entry(EntryState::Absent);
        let old = fx.state.file_report.rev;
        fx.put_ok(json!({"socks": {"port": 1090}}));
        // when
        let (status, v) = fx.put_at(old, enable());
        // then
        assert_eq!(status, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["error"], "stale_rev", "{v}");
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_invalid_other_field_does_not_touch_entry() {
        let mut fx = entry(EntryState::Absent);
        let (status, v) = fx.put(json!({"autostart": {"enabled": true}, "socks": {"port": 80}}));
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert_eq!(v["error"], "invalid", "{v}");
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }

    #[test]
    fn put_autostart_skips_apply_config() {
        // given: an immediate field is saved together with the entry
        let mut fx = entry(EntryState::Absent);
        // when
        fx.put_ok(json!({"autostart": {"enabled": true}, "checks": {"interval_sec": 600}}));
        // then: the entry changed synchronously, the node is told only about the other field
        assert_eq!(fx.changes(), ["enable"]);
        assert_eq!(
            *fx.node.applied.lock().unwrap(),
            vec![vec!["checks.interval_sec".to_string()]]
        );
    }
}

mod added_slot {
    use super::*;

    #[test]
    fn slot_get_shows_system_state() {
        let mut fx = entry(EntryState::Current);
        fx.make_slot();
        assert_eq!(fx.shown(), view(true, true, ""));
    }

    #[test]
    fn slot_put_never_touches_entry() {
        // given: a slot with a network, the entry is absent
        let mut fx = entry(EntryState::Absent);
        fx.make_slot();
        // when: the body asks for `true`
        fx.put_ok(enable());
        // then
        assert!(fx.changes().is_empty(), "calls: {:?}", fx.changes());
    }
}

struct Fixture {
    dir: PathBuf,
    state: ConfigState,
    node: StubNode,
    fake: Option<&'static FakeAutostart>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        cleanup(&self.dir);
    }
}

impl Fixture {
    /// The `autostart` section of what `GET /api/config` returns.
    fn shown(&self) -> Value {
        self.state.config_get_body()["config"]["autostart"].clone()
    }

    fn put(&mut self, config: Value) -> (StatusCode, Value) {
        let rev = self.state.file_report.rev;
        self.put_at(rev, config)
    }

    fn put_at(&mut self, rev: u64, config: Value) -> (StatusCode, Value) {
        body_json(self.state.put(&put_body(rev, config), &self.node))
    }

    fn put_ok(&mut self, config: Value) -> Value {
        let (status, v) = self.put(config);
        assert_eq!(status, StatusCode::OK, "{v}");
        v
    }

    /// Methods of the backend that changed the entry (`state` reads are not counted).
    fn changes(&self) -> Vec<&'static str> {
        let calls = self.fake.map(|f| f.calls()).unwrap_or_default();
        calls.into_iter().filter(|call| *call != "state").collect()
    }

    fn file_text(&self) -> String {
        std::fs::read_to_string(self.dir.join("config.json")).unwrap_or_default()
    }

    /// The `autostart.enabled` choice as a fresh start would read it from the file.
    fn saved_flag(&self) -> bool {
        new_state(&self.dir).file_settings.process.autostart_enabled
    }

    /// Turns the state into an added slot that has a network.
    fn make_slot(&mut self) {
        self.state.current_network_key = Some("key".to_string());
        let process = self.state.file_settings.process.clone();
        self.state.overlay_process(&process);
    }
}

/// A state with a fresh settings file and the given backend (`None` — a build without the tray).
fn setup(fake: Option<&'static FakeAutostart>) -> Fixture {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    state.autostart = fake.map(|f| f as &'static dyn AutostartBackend);
    Fixture {
        dir,
        state,
        node: StubNode::new(),
        fake,
    }
}

fn entry(state: EntryState) -> Fixture {
    setup(Some(leaked(state)))
}

/// The file already says `true` (saved through a throwaway backend); the node restarted with `fake`.
fn setup_saved_on(fake: Option<&'static FakeAutostart>) -> Fixture {
    let mut fx = entry(EntryState::Absent);
    fx.put_ok(enable());
    fx.state = new_state(&fx.dir);
    fx.state.autostart = fake.map(|f| f as &'static dyn AutostartBackend);
    fx.fake = fake;
    fx
}

/// A 422 `not_available` whose top-level message and `autostart.enabled` entry carry `text`.
fn assert_refused(status: StatusCode, v: &Value, text: &str) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["error"], "not_available", "{v}");
    assert_eq!(v["message"], text, "{v}");
    let field = json!({"path": "autostart.enabled", "message": text});
    assert_eq!(v["fields"][0], field, "{v}");
}

fn view(enabled: bool, available: bool, reason: &str) -> Value {
    json!({"enabled": enabled, "available": available, "reason": reason})
}

fn enable() -> Value {
    json!({"autostart": {"enabled": true}})
}

fn disable() -> Value {
    json!({"autostart": {"enabled": false}})
}
