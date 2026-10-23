//! Starting a network from an optional config: with a key the node boots at once, without one it waits
//! for a key; both ways a stop request ends the call with `Ok`. All tests go through `start_network`,
//! the only place that knows which start function is behind it.

use super::*;

const LEAVE_WITHIN: Duration = Duration::from_secs(20);

mod with_key {
    use super::*;

    /// given a config, when the stop was requested before the start, then the call ends with `Ok`.
    #[tokio::test]
    async fn stop_before_start_ends_ok() {
        // given
        let fx = setup_with_key(61);
        fx.ctl.stop_on_os_signal();

        // when
        let res = timeout(LEAVE_WITHIN, fx.start()).await;

        // then
        assert!(matches!(res, Ok(Ok(()))), "stop before start: {res:?}");
    }

    /// given a config, when the stop was requested before the start, then the node still boots once.
    #[tokio::test]
    async fn stop_before_start_still_boots_once() {
        // given
        let fx = setup_with_key(62);
        let id0 = fx.ctl.snapshot().boot_id;
        fx.ctl.stop_on_os_signal();

        // when
        let res = timeout(LEAVE_WITHIN, fx.start()).await;

        // then
        assert!(matches!(res, Ok(Ok(()))), "{res:?}");
        assert_ne!(id0, fx.ctl.snapshot().boot_id, "the node must boot once");
    }

    /// given a running node, when the stop is requested, then the call ends with `Ok`.
    #[tokio::test]
    async fn stop_while_running_ends_ok() {
        // given
        let fx = setup_with_key(63);
        let ctl = fx.ctl.clone();
        let task = crate::node::output::spawn(fx.start());
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!task.is_finished(), "a started node keeps running");

        // when
        ctl.stop();

        // then
        let res = timeout(LEAVE_WITHIN, task).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "stop while running: {res:?}");
    }

    /// given a running node, when it runs, then it does not wait for a network key.
    #[tokio::test]
    async fn running_node_is_not_without_network() {
        // given
        let fx = setup_with_key(64);
        let ctl = fx.ctl.clone();
        let task = crate::node::output::spawn(fx.start());

        // when
        tokio::time::sleep(Duration::from_millis(300)).await;

        // then
        assert!(!ctl.is_no_network(), "a node with a key has a network");
        ctl.stop();
        let res = timeout(LEAVE_WITHIN, task).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "{res:?}");
    }
}

mod without_key {
    use super::*;

    /// given no config, when the stop was requested before the call, then the call ends with `Ok`.
    #[tokio::test]
    async fn stop_before_start_ends_ok() {
        // given
        let fx = setup_without_key();
        fx.ctl.stop();

        // when
        let res = timeout(LEAVE_WITHIN, fx.start()).await;

        // then
        assert!(matches!(res, Ok(Ok(()))), "stop before start: {res:?}");
    }

    /// given no config, when the node runs, then it waits for a key (`NoNetwork`) until it is stopped,
    /// and the stop ends the call with `Ok`.
    #[tokio::test]
    async fn waits_for_key_until_stopped() {
        // given
        let fx = setup_without_key();
        let ctl = fx.ctl.clone();
        let task = crate::node::output::spawn(fx.start());

        // when
        tokio::time::sleep(Duration::from_millis(100)).await;
        let waiting = ctl.is_no_network();
        let finished = task.is_finished();
        ctl.stop();

        // then
        assert!(waiting, "the controller must be in `NoNetwork` while it waits");
        assert!(!finished, "the call must not end before the stop");
        let res = timeout(LEAVE_WITHIN, task).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "stop while waiting: {res:?}");
    }

    /// given no config, when a bad key arrives, then the node keeps waiting and a stop still ends it with `Ok`.
    #[tokio::test]
    async fn bad_key_keeps_waiting() {
        // given
        let fx = setup_without_key();
        let ctl = fx.ctl.clone();
        let task = crate::node::output::spawn(fx.start());

        // when
        ctl.join_with_peers("not a key at all", vec![]);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // then
        assert!(!task.is_finished(), "a bad key must not end the call");
        assert!(ctl.is_no_network(), "back to waiting after a bad key");
        ctl.stop();
        let res = timeout(LEAVE_WITHIN, task).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "{res:?}");
    }
}

struct Fixture {
    start: Option<NodeConfig>,
    settings: Settings,
    ctl: Arc<NodeCtl>,
}

impl Fixture {
    fn start(&self) -> impl std::future::Future<Output = Result<()>> {
        start_network(
            self.start.clone(),
            self.settings.clone(),
            self.ctl.clone(),
            test_relaunch(),
        )
    }
}

fn setup_with_key(seed: u8) -> Fixture {
    let settings = Settings::default();
    Fixture {
        start: Some(NodeConfig::for_loopback_test(NetworkKey::from_bytes(
            [seed; 32],
        ))),
        ctl: NodeCtl::new(&settings),
        settings,
    }
}

fn setup_without_key() -> Fixture {
    let settings = Settings::default();
    Fixture {
        start: None,
        ctl: NodeCtl::new_without_network(&settings),
        settings,
    }
}

/// The single entry the tests start a network through: `Some` starts it at once, `None` waits for a key.
async fn start_network(
    start: Option<NodeConfig>,
    settings: Settings,
    ctl: Arc<NodeCtl>,
    relaunch: Relaunch,
) -> Result<()> {
    match start {
        Some(cfg) => run_node_with_ctl(cfg, settings, ctl, relaunch).await,
        None => {
            let identity = IdentityState::Temporary(TemporaryReason::Declined);
            run_node_awaiting_key(settings, identity, ctl, relaunch).await
        }
    }
}
