//! The fall back to waiting for a network after a failed start: the phase, the saved reason, and how a
//! restart, a stop and a new join treat it.

use std::sync::Arc;
use std::time::Duration;

use super::super::state::{Cmd, NodeCtl, Phase};
use crate::config::config_file::{NetSettings, Settings};
use crate::web_gui::{ActionError, NodeControl, NodeStatus};

const KEY_TEXT: &str = "label--0123456789abcdef";
const REASON: &str = "port 1080 is busy";

mod fall_back_to_no_network {
    use super::*;

    #[tokio::test]
    async fn fall_back_turns_stopping_into_no_network_with_the_reason() {
        let ctl = ctl_in(Phase::Stopping);

        ctl.fall_back_to_no_network(REASON.to_string());

        assert_eq!(phase_name(&ctl), "NoNetwork");
        assert_eq!(ctl.start_error(), Some(REASON.to_string()));
    }

    #[tokio::test]
    async fn fall_back_from_starting_too() {
        let ctl = ctl_in(Phase::Starting);

        ctl.fall_back_to_no_network(REASON.to_string());

        assert_eq!(phase_name(&ctl), "NoNetwork");
        assert_eq!(ctl.start_error(), Some(REASON.to_string()));
    }

    #[tokio::test]
    async fn fall_back_leaves_up_alone() {
        let ctl = ctl_in(Phase::Up);

        ctl.fall_back_to_no_network(REASON.to_string());

        assert_eq!(phase_name(&ctl), "Up", "a running node stays up");
        assert_eq!(ctl.start_error(), None);
    }
}

mod the_reason {
    use super::*;

    #[tokio::test]
    async fn is_in_the_snapshot_while_waiting_for_a_network() {
        let ctl = ctl_in(Phase::Starting);

        ctl.fall_back_to_no_network(REASON.to_string());

        let node = ctl.snapshot().node;
        assert_eq!(node.status, NodeStatus::Offline);
        assert_eq!(node.start_error, Some(REASON.to_string()));
    }

    #[tokio::test]
    async fn is_null_in_the_snapshot_when_nothing_failed() {
        let ctl = ctl_in(Phase::NoNetwork);

        assert_eq!(ctl.snapshot().node.start_error, None);
    }

    #[tokio::test]
    async fn mark_starting_clears_the_reason() {
        let ctl = ctl_in(Phase::Starting);
        ctl.fall_back_to_no_network(REASON.to_string());

        ctl.mark_starting();

        // the phase filter of `start_error()` would hide a stale value: look at it in `NoNetwork` again
        set_phase(&ctl, Phase::NoNetwork);
        assert_eq!(ctl.start_error(), None, "the old reason must not come back");
    }

    #[tokio::test]
    async fn a_new_join_hides_the_old_reason() {
        let ctl = ctl_in(Phase::Starting);
        ctl.fall_back_to_no_network(REASON.to_string());

        ctl.switch_network(KEY_TEXT);

        assert_eq!(phase_name(&ctl), "Starting");
        assert_eq!(ctl.start_error(), None);
        assert_eq!(ctl.snapshot().node.start_error, None);
    }

    #[tokio::test]
    async fn restart_clears_the_reason() {
        let ctl = ctl_with_key(Phase::Starting);
        ctl.fall_back_to_no_network(REASON.to_string());
        ctl.request_restart()
            .expect("a restart after a failed start");

        set_phase(&ctl, Phase::NoNetwork);

        assert_eq!(ctl.start_error(), None, "the restart replaces the reason");
    }

    #[tokio::test]
    async fn survives_a_refused_restart() {
        let ctl = ctl_in(Phase::Starting);
        ctl.fall_back_to_no_network(REASON.to_string());

        let res = ctl.request_restart();

        assert!(matches!(res, Err(ActionError::Internal(_))), "got {res:?}");
        assert_eq!(ctl.start_error(), Some(REASON.to_string()));
    }
}

mod restart_after_a_failed_start {
    use super::*;

    #[tokio::test]
    async fn is_not_refused_as_no_network() {
        let ctl = ctl_with_key(Phase::Starting);
        ctl.fall_back_to_no_network(REASON.to_string());

        let res = ctl.request_restart();

        assert_eq!(res, Ok(()));
        assert_eq!(phase_name(&ctl), "Starting");
    }
}

mod stop_and_the_fall_back {
    use super::*;

    /// given a stop requested while the start was failing, when the start falls back, then the stop is still
    /// the next command: the wait loop sees it.
    #[tokio::test]
    async fn a_stop_before_the_fall_back_is_not_lost() {
        let ctl = ctl_in(Phase::Starting);
        ctl.request_stop();

        ctl.fall_back_to_no_network(REASON.to_string());

        let next = tokio::time::timeout(Duration::from_secs(2), ctl.next_command()).await;
        assert!(matches!(next, Ok(Cmd::Stop)), "got {next:?}");
    }
}

/// A controller with no key in the settings; the phase is set directly (there is no node).
fn ctl_in(phase: Phase) -> Arc<NodeCtl> {
    let ctl = NodeCtl::new(&Settings::default());
    set_phase(&ctl, phase);
    ctl
}

/// A controller whose settings carry a network key, so that a restart has something to restart into.
fn ctl_with_key(phase: Phase) -> Arc<NodeCtl> {
    let settings = Settings {
        net: NetSettings {
            network_key: Some(KEY_TEXT.to_string()),
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let ctl = NodeCtl::new(&settings);
    set_phase(&ctl, phase);
    ctl
}

fn set_phase(ctl: &NodeCtl, phase: Phase) {
    ctl.ctl_shared.ctl_state.lock().unwrap().phase = phase;
}

fn phase_name(ctl: &NodeCtl) -> &'static str {
    match ctl.ctl_shared.ctl_state.lock().unwrap().phase {
        Phase::NoNetwork => "NoNetwork",
        Phase::Starting => "Starting",
        Phase::Up => "Up",
        Phase::Stopping => "Stopping",
    }
}
