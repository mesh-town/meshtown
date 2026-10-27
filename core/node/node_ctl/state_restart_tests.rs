use super::*;

const KEY_TEXT: &str = "label--0123456789abcdef";

mod from_up_with_key {
    use super::*;

    #[tokio::test]
    async fn restart_from_up_sends_switch_with_current_key() {
        // given
        let ctl = setup(Phase::Up, Some(KEY_TEXT));
        // when
        let res = ctl.request_restart();
        // then
        assert_eq!(res, Ok(()));
        assert_eq!(
            phase_name(&ctl),
            "Starting",
            "the phase must become Starting"
        );
        assert_eq!(
            ctl.next_command().await,
            Cmd::Switch {
                key: KEY_TEXT.to_string(),
                peers: vec![]
            }
        );
    }
}

mod after_a_failed_start {
    use super::*;

    /// given a start that failed and left the node waiting with the saved key, when the page restarts it,
    /// then the key is sent again in a `Switch`.
    #[tokio::test]
    async fn restart_after_a_failed_start_sends_switch_with_the_saved_key() {
        // given
        let ctl = setup(Phase::Starting, Some(KEY_TEXT));
        ctl.fall_back_to_no_network("port is busy".to_string());
        // when
        let res = ctl.request_restart();
        // then
        assert_eq!(res, Ok(()));
        assert_eq!(phase_name(&ctl), "Starting");
        assert_eq!(
            ctl.next_command().await,
            Cmd::Switch {
                key: KEY_TEXT.to_string(),
                peers: vec![]
            }
        );
    }
}

mod refused {
    use super::*;

    #[tokio::test]
    async fn restart_from_no_network_is_no_network_not_busy() {
        let ctl = NodeCtl::new_without_network(&Settings::default());

        let res = ctl.request_restart();

        assert!(matches!(res, Err(ActionError::NoNetwork(_))), "got {res:?}");
        assert_eq!(phase_name(&ctl), "NoNetwork");
        assert_channel_empty(&ctl).await;
    }

    #[tokio::test]
    async fn restart_while_starting_is_busy() {
        let ctl = NodeCtl::new(&Settings::default());

        let res = ctl.request_restart();

        assert_eq!(
            res,
            Err(ActionError::Busy("Mesh Town is starting".to_string()))
        );
        assert_channel_empty(&ctl).await;
    }

    #[tokio::test]
    async fn restart_while_stopping_is_busy() {
        let ctl = setup(Phase::Up, Some(KEY_TEXT));
        ctl.request_stop();

        let res = ctl.request_restart();

        assert_eq!(
            res,
            Err(ActionError::Busy("Mesh Town is stopping".to_string()))
        );
        assert_eq!(ctl.next_command().await, Cmd::Stop);
    }

    #[tokio::test]
    async fn restart_without_key_is_internal() {
        let ctl = setup(Phase::Up, None);

        let res = ctl.request_restart();

        assert!(matches!(res, Err(ActionError::Internal(_))), "got {res:?}");
        assert_eq!(phase_name(&ctl), "Up", "the phase must not change");
        assert_channel_empty(&ctl).await;
    }
}

/// `NodeCtl::new` with the network key `key` in the settings; the phase is set directly (there is no node).
fn setup(phase: Phase, key: Option<&str>) -> Arc<NodeCtl> {
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            network_key: key.map(str::to_string),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let ctl = NodeCtl::new(&settings);
    ctl.ctl_shared.ctl_state.lock().unwrap().phase = phase;
    ctl
}

fn phase_name(ctl: &NodeCtl) -> &'static str {
    match ctl.ctl_shared.ctl_state.lock().unwrap().phase {
        Phase::NoNetwork => "NoNetwork",
        Phase::Starting => "Starting",
        Phase::Up => "Up",
        Phase::Stopping => "Stopping",
    }
}

async fn assert_channel_empty(ctl: &NodeCtl) {
    let res = tokio::time::timeout(Duration::from_millis(50), ctl.next_command()).await;
    assert!(res.is_err(), "there must be nothing in the command channel");
}
