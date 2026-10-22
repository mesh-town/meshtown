//! Tests for `poller_exit`: the pure edit of the selected exits list on a command from the tray submenu.

use super::*;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::transport::net::NodeSecret;
use serde_json::Value;

mod edit_exits {
    use super::*;

    fn v(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn toggle_new_id_replaces_list() {
        let mut list = v(&["a", "b"]);

        edit_exits(&mut list, &ExitEdit::Toggle("c".to_string()));

        assert_eq!(list, v(&["c"]));
    }

    #[test]
    fn toggle_on_empty_list_selects_it() {
        let mut list = v(&[]);

        edit_exits(&mut list, &ExitEdit::Toggle("a".to_string()));

        assert_eq!(list, v(&["a"]));
    }

    #[test]
    fn toggle_selected_id_clears_list() {
        let mut two = v(&["a", "b"]);
        let mut one = v(&["b"]);

        edit_exits(&mut two, &ExitEdit::Toggle("b".to_string()));
        edit_exits(&mut one, &ExitEdit::Toggle("b".to_string()));

        assert!(two.is_empty(), "{two:?}");
        assert!(one.is_empty(), "{one:?}");
    }

    #[test]
    fn clear_empties_the_list() {
        let mut list = vec!["a".to_string(), "b".to_string()];

        edit_exits(&mut list, &ExitEdit::Clear);

        assert!(list.is_empty(), "Clear must leave an empty list: {list:?}");
    }
}

// The `send_exit_edit` round (GET /api/config → edit → PUT with rev) runs against real HTTP through
// `start_fake_with_config`. A retry on a stale `rev` is not tested separately: `try_edit_once`
// makes `GET` and `PUT` as two sequential requests within one `await` stack, and the fixture
// offers no hook to wedge in between them and swap `rev` deterministically (not through a real
// thread race).
mod send_exit_edit_round_trip {
    use super::*;

    use std::time::Duration;

    use crate::gui::tray::app::local_api::tests::{start_fake_with_config, temp_dir, FAKE_TOKEN};
    use crate::gui::tray::app::tray_role::poll_types::PollTiming;

    const T: PollTiming = PollTiming {
        poll_interval: Duration::from_millis(100),
        own_unavailable_after: Duration::from_secs(1),
        stop_wait: Duration::from_secs(1),
        foreign_gone_after_failures: 3,
        note_visible_for: Duration::from_millis(400),
    };

    /// Starts a fake node with a real config file and a `Poller` that reads the same token from
    /// the file (`send_exit_edit` takes the token through `read_token(&self.token_path)`).
    async fn rig() -> (crate::web_gui::WebHandle, std::path::PathBuf, Poller) {
        let dir = temp_dir("poller-exit-edit");
        let (handle, _node) = start_fake_with_config(0, &dir).await;
        let port = handle.shared.port();

        let token_path = dir.join("token");
        std::fs::write(&token_path, FAKE_TOKEN).expect("write the token file for the test");

        let poller = Poller {
            port,
            proxy: ProxyState::Off(ProxyOff::DisabledInSettings),
            token_path: token_path.clone(),
            spawn: Box::new(|| panic!("spawn must not be called in the exits edit test")),
            send_view: Box::new(|_v| {}),
            send_exit_menu: Box::new(|_m| {}),
            send_invite_note: Box::new(|_n| {}),
            send_serve_note: Box::new(|_n| {}),
            copy_to_clipboard_fn: std::sync::Arc::new(|_t: &str| Ok(())),
            log: Box::new(|_s| {}),
            last_view: None,
            last_exit_menu: None,
            notes_visible_until: None,
            start_fail: crate::gui::tray::app::tray_role::start_fail::StartFail::new(
                Box::new(|| None),
                Box::new(|| None),
                Box::new(|_n| {}),
            ),
            timing: T,
        };
        (handle, dir, poller)
    }

    async fn current_exits(port: u16) -> Vec<String> {
        let reply = local_api::get_config(port, FAKE_TOKEN)
            .await
            .expect("get_config: the exchange must not fail");
        match reply {
            Reply::Ok(value) => current_exits_impl(&value),
            other => panic!("expected Reply::Ok, got {other:?}"),
        }
    }

    /// `value` is the whole `GET /api/config` response (`{"rev":…, "config": {"route": {"exits": […]}}…}`,
    /// see `ConfigState::config_get_body`): the settings document itself lies under the `config` key, not at
    /// the top level of the response.
    fn current_exits_impl(value: &Value) -> Vec<String> {
        value
            .get("config")
            .and_then(|c| c.get("route"))
            .and_then(|r| r.get("exits"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `route.exits` is parsed by the server as a list of `PeerId` (`take_endpoint_id_list`,
    /// `config_file/parse_fields.rs`), not of arbitrary strings: a value like `"newexit"` does not
    /// parse, the whole list rolls back to `[]` with a `field_error` on `route.exits`, and
    /// `PUT /api/config` answers `invalid` instead of `Ok` — which looks like "the edit was not applied", although
    /// a real node never gets such an id from the submenu (there ids come from
    /// `peers[]`/`route.exits[]`). Test ids must be real `PeerId`s.
    fn id(byte: u8) -> String {
        NodeSecret::from_bytes(&[byte; 32]).peer_id().to_string()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_new_id_appears_in_config() {
        let (handle, dir, mut poller) = rig().await;
        let port = poller.port;
        let new_id = id(1);

        poller
            .send_exit_edit(ExitEdit::Toggle(new_id.clone()))
            .await;

        assert_eq!(
            current_exits(port).await,
            vec![new_id],
            "Toggle of a new id must add it to the config on the fake"
        );

        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn repeated_toggle_removes_it() {
        let (handle, dir, mut poller) = rig().await;
        let port = poller.port;
        let new_id = id(1);

        poller
            .send_exit_edit(ExitEdit::Toggle(new_id.clone()))
            .await;
        poller.send_exit_edit(ExitEdit::Toggle(new_id)).await;

        assert!(
            current_exits(port).await.is_empty(),
            "a repeated Toggle of the same id must remove it again"
        );

        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn second_toggle_replaces_first_in_config() {
        let (handle, dir, mut poller) = rig().await;
        let port = poller.port;
        let (id1, id2) = (id(1), id(2));

        poller.send_exit_edit(ExitEdit::Toggle(id1)).await;
        poller.send_exit_edit(ExitEdit::Toggle(id2.clone())).await;

        // a second Toggle of another id replaces the first rather than accumulating — like the button on the page
        assert_eq!(
            current_exits(port).await,
            vec![id2],
            "a second Toggle of another id must replace the first"
        );

        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn clear_empties_config() {
        let (handle, dir, mut poller) = rig().await;
        let port = poller.port;

        poller.send_exit_edit(ExitEdit::Toggle(id(1))).await;
        poller.send_exit_edit(ExitEdit::Clear).await;

        assert!(
            current_exits(port).await.is_empty(),
            "Clear must empty the config on the fake"
        );

        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
