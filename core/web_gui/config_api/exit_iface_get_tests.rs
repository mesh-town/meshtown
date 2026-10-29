//! `exit.interface` clearing by `null` and its `GET /api/config` view. The harness is from
//! `mod_tests.rs` via `use super::*;`, the fixture from `exit_iface_put_tests.rs`.

use super::exit_iface_put_tests::{named, setup, strings, PATH};
use super::*;

mod put_null_clears {
    use super::*;

    #[test]
    fn the_key_leaves_the_file() {
        // given: a saved name
        let mut f = setup();
        f.when_put(named("wg0"));
        // when
        let (status, v) = f.when_put(serde_json::json!({"exit": {"interface": null}}));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert!(
            !f.main_text().contains("\"interface\""),
            "{}",
            f.main_text()
        );
        assert_eq!(f.main_file_value(), None);
    }

    #[test]
    fn the_path_is_changed_and_applied() {
        // given
        let mut f = setup();
        f.when_put(named("wg0"));
        // when
        let (_, v) = f.when_put(serde_json::json!({"exit": {"interface": null}}));
        // then
        assert_eq!(strings(&v["changed"]), [PATH]);
        assert_eq!(f.applied().last(), Some(&vec![PATH.to_string()]));
        assert_eq!(f.state.file_settings.net.exit_interface, None);
    }
}

mod get_shows_interface {
    use super::*;

    #[test]
    fn the_name_when_set() {
        // given
        let mut f = setup();
        f.when_put(named("wg0"));
        // when
        let body = f.state.config_get_body();
        // then
        assert_eq!(body["config"]["exit"]["interface"], "wg0");
    }

    #[test]
    fn no_key_when_unset() {
        // given
        let f = setup();
        // when
        let body = f.state.config_get_body();
        // then
        assert!(
            body["config"]["exit"].get("interface").is_none(),
            "{}",
            body["config"]
        );
    }

    #[test]
    fn the_path_is_editable_not_read_only() {
        // given
        let f = setup();
        // when
        let body = f.state.config_get_body();
        // then
        assert!(
            !strings(&body["read_only"]).contains(&PATH.to_string()),
            "{body}"
        );
    }
}
