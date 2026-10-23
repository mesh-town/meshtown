//! Tests for `poller_exit`: the "Allow sites to open through this computer" checkbox.

use super::*;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use serde_json::Value;

// The "Allow sites to open through this computer" checkbox: the edit body, the write through the
// GET → PUT round, and the menu converging to the snapshot.
mod serve_checkbox {
    use super::*;

    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::gui::tray::app::exit_menu::disabled_exit_menu;
    use crate::gui::tray::app::local_api::tests::{
        start_fake, start_fake_with_config, temp_dir, FAKE_TOKEN,
    };
    use crate::gui::tray::app::tray_actions::ServeNote;
    use crate::gui::tray::app::tray_role::poll_types::PollTiming;
    use crate::gui::tray::app::tray_role::poller::test_support::*;
    use crate::gui::tray::app::tray_texts::REASON_SERVE_REFUSED;
    use crate::gui::tray::NodeCmd;

    const LOG_LINE: &str = "permission to open sites: edit was not applied";

    fn poller(port: u16, token_path: std::path::PathBuf, log: Arc<Mutex<Vec<String>>>) -> Poller {
        Poller {
            port,
            token_path,
            spawn: Box::new(|| panic!("spawn must not be called")),
            send_view: Box::new(|_v| {}),
            send_exit_menu: Box::new(|_m| {}),
            send_invite_note: Box::new(|_n| {}),
            send_serve_note: Box::new(|_n| {}),
            copy_to_clipboard_fn: Arc::new(|_t: &str| Ok(())),
            log: Box::new(move |s| log.lock().unwrap().push(s)),
            last_view: None,
            proxy: ProxyState::Off(ProxyOff::DisabledInSettings),
            last_exit_menu: Some(disabled_exit_menu(ProxyState::Off(
                ProxyOff::DisabledInSettings,
            ))),
            notes_visible_until: None,
            start_fail: crate::gui::tray::app::tray_role::start_fail::StartFail::new(
                Box::new(|| None),
                Box::new(|| None),
                Box::new(|_n| {}),
            ),
            timing: PollTiming::PROD,
            overview_gate: crate::gui::tray::app::tray_role::overview_poll::OverviewGate::new(),
        }
    }

    async fn config_exit_enabled(port: u16) -> bool {
        match local_api::get_config(port, FAKE_TOKEN, None).await {
            Ok(Reply::Ok(v)) => v["config"]["exit"]["enabled"]
                .as_bool()
                .expect("config.exit.enabled is in the reply"),
            other => panic!("expected Reply::Ok, got {other:?}"),
        }
    }

    #[test]
    fn exit_server_body_is_patch() {
        let body = exit_server_body(7, false);

        let parsed: Value = serde_json::from_slice(&body).expect("the body is JSON");
        let want = serde_json::json!({"rev": 7, "config": {"exit": {"enabled": false}}});
        assert_eq!(parsed, want);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_exit_server_writes_config() {
        let dir = temp_dir("poller-serve-write");
        let (handle, _node) = start_fake_with_config(0, &dir).await;
        let port = handle.shared.port();
        std::fs::write(dir.join("token"), FAKE_TOKEN).unwrap();
        let mut p = poller(port, dir.join("token"), Arc::default());

        p.send_exit_server(None, false).await;
        let after_off = config_exit_enabled(port).await;
        p.send_exit_server(None, true).await;
        let after_on = config_exit_enabled(port).await;

        assert!(!after_off, "after false the config must hold false");
        assert!(after_on, "after true the config must hold true");
        assert_eq!(p.last_exit_menu, None);
        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_edit_forgets_last_exit_menu() {
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let mut p = poller(
            1,
            std::path::PathBuf::from("/nonexistent/token"),
            log.clone(),
        );

        p.send_exit_server(None, true).await;

        assert_eq!(p.last_exit_menu, None);
        assert_eq!(*log.lock().unwrap(), vec![LOG_LINE.to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serve_click_converges_to_snapshot() {
        let dir = temp_dir("poller-serve-converge");
        let (handle, _node) = start_fake_with_config(0, &dir).await;
        let port = handle.shared.port();
        let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
        rig.probe.exits("checkbox is on", |m| m.serves_exit);

        rig.cmd(NodeCmd::SetExitServer { slot: None, on: false });
        let menu = rig.probe.exits("checkbox cleared by the snapshot", |m| {
            m.enabled && !m.serves_exit
        });

        assert!(menu.enabled);
        assert!(!config_exit_enabled(port).await);
        rig.quit();
        assert_eq!(rig.code().await, 0);
        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_serve_click_resends_same_model() {
        let dir = temp_dir("poller-serve-fail");
        let (handle, _node) = start_fake(0, &dir).await;
        let port = handle.shared.port();
        let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
        let before = rig.probe.exits("first model", |m| m.enabled);

        rig.cmd(NodeCmd::SetExitServer { slot: None, on: !before.serves_exit });
        rig.probe.log(LOG_LINE);
        let again = rig.probe.exits("model sent again", |m| m.enabled);

        assert_eq!(
            again, before,
            "the snapshot did not change — the model is the same"
        );
        rig.quit();
        assert_eq!(rig.code().await, 0);
        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_serve_shows_note_then_idle() {
        let dir = temp_dir("poller-serve-note");
        let (handle, _node) = start_fake(0, &dir).await;
        let port = handle.shared.port();
        let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
        let before = rig.probe.exits("first model", |m| m.enabled);
        let want = ServeNote::Failed {
            was_enabling: !before.serves_exit,
            reason: REASON_SERVE_REFUSED,
        };

        rig.cmd(NodeCmd::SetExitServer { slot: None, on: !before.serves_exit });
        let shown = rig.probe.serve("failure", |n| *n != ServeNote::Idle);
        let at = std::time::Instant::now();
        rig.probe.serve("rest", |n| *n == ServeNote::Idle);

        assert_eq!(shown, want);
        assert!(
            at.elapsed() >= Duration::from_millis(300),
            "Idle too early: {:?}",
            at.elapsed()
        );
        rig.quit();
        assert_eq!(rig.code().await, 0);
        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn successful_serve_clears_note() {
        let dir = temp_dir("poller-serve-ok-note");
        let (handle, _node) = start_fake_with_config(0, &dir).await;
        let port = handle.shared.port();
        let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
        rig.probe.exits("checkbox is on", |m| m.serves_exit);

        rig.cmd(NodeCmd::SetExitServer { slot: None, on: false });
        rig.probe
            .serve("rest after success", |n| *n == ServeNote::Idle);
        rig.probe.exits("checkbox cleared by the snapshot", |m| {
            m.enabled && !m.serves_exit
        });

        let failed = rig
            .probe
            .history
            .iter()
            .any(|s| matches!(s, Seen::Serve(ServeNote::Failed { .. })));
        assert!(
            !failed,
            "a successful edit must not show a failure: {:?}",
            rig.probe.history
        );
        rig.quit();
        assert_eq!(rig.code().await, 0);
        handle.shutdown(Duration::from_secs(2)).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serve_click_outside_online_phase_is_dropped() {
        let dir = temp_dir("poller-serve-dropped");
        let mut rig = Rig::start(1, dir.join("web-token"), false, spawn_panics());
        let disabled = crate::gui::tray::app::tray_role::views::grey_view(
            crate::gui::tray::app::view::PageState::Disabled,
            crate::gui::tray::app::view::NodeState::Unknown,
        );
        rig.probe.view("Disabled", |v| *v == disabled);

        rig.cmd(NodeCmd::SetExitServer { slot: None, on: true });
        tokio::time::sleep(T.note_visible_for * 2).await;

        assert!(
            !rig.probe.logged(LOG_LINE),
            "a command outside the online phase must be silently dropped"
        );
        rig.quit();
        assert_eq!(rig.code().await, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
