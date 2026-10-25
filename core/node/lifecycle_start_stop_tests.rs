//! A stop that arrives while the node starts, and the commands that must not interrupt a start.
//!
//! A hanging start is a future that never ends; a gated start is a real loopback node whose start is held
//! until the test opens the gate. The tests run on two threads with real time and an outer `timeout`.

use std::future::Future;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};

use super::start_race::STOP_POLL;
use super::tests::proxy_support::hold_replica_flush;
use super::tests::{key_text, test_relaunch};
use super::*;
use crate::config::config_file::Settings;
use crate::members::network_key::NetworkKey;
use crate::node::identity::TemporaryReason;
use crate::transport::net::NodeSecret;
use crate::web_gui::{NodeControl, NodeStatus};

const OUTER_LIMIT: Duration = Duration::from_secs(5);
const STOP_AFTER: Duration = Duration::from_millis(20);

mod stop_during_start {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_during_start_on_the_first_cycle_returns_none() {
        let fx = setup();

        let res = fx.when_stopped_during_a_hanging_start(true).await;

        assert!(res.unwrap().is_none(), "the stop must interrupt the start");
        fx.then_status_is(NodeStatus::Stopping);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_during_start_on_a_repeat_cycle_returns_none() {
        let fx = setup();

        let res = fx.when_stopped_during_a_hanging_start(false).await;

        assert!(res.is_ok(), "an interrupted start is not a failed cycle");
        assert!(res.unwrap().is_none(), "the stop must interrupt the start");
    }
}

mod start_error {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_error_still_goes_through_cycle_failed() {
        // given: a start that fails
        let fx = setup();
        let start = async { Err::<RunningNode, _>(anyhow::anyhow!("no start")) };

        // when
        let res = boot_cycle_with(&fx.ctl, true, start).await;

        // then: the error comes out and the node is marked as stopping
        assert!(res.is_err(), "the start error must come out");
        fx.then_status_is(NodeStatus::Stopping);
    }
}

mod shutdown {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_waits_for_the_replica_flush() {
        // given: a running node whose replica flush is held
        let mut node = start_node(loopback_config(31))
            .await
            .expect("loopback start");
        let held = hold_replica_flush(&mut node).await;

        // when: the node is shut down
        let mut ending = Box::pin(node.shutdown());

        // then: it does not end while the flush is held, and ends once the flush is released
        let early = timeout(Duration::from_millis(300), &mut ending).await;
        assert!(
            early.is_err(),
            "the shutdown must wait for the replica flush"
        );
        held.close();
        timeout(OUTER_LIMIT, ending)
            .await
            .expect("the shutdown must end after the flush");
    }
}

mod commands_that_do_not_interrupt_a_start {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn leave_net_during_start_waits_for_the_start_and_is_served_after() {
        // given: a start held behind a gate and a request to leave the network
        let fx = setup();
        let secret = NodeSecret::from_bytes(&[7; 32]);
        fx.ctl
            .request_leave_net(secret.clone())
            .expect("leave request");

        // when: the start is let go after several stop checks
        let node = fx.when_start_finishes_after_five_checks(32).await;

        // then: the node comes up, and the leave command is served afterwards
        let cmd = fx.serve(node).await;
        assert_eq!(cmd, Cmd::LeaveNet { secret });
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn switch_during_start_waits_for_the_start_and_is_served_after() {
        // given: a start held behind a gate and a request to switch the network
        let fx = setup();
        fx.ctl.switch_network(&key_text(9));

        // when: the start is let go after several stop checks
        let node = fx.when_start_finishes_after_five_checks(33).await;

        // then: the node comes up, and the switch is served afterwards
        let cmd = fx.serve(node).await;
        let want = Cmd::Switch {
            key: key_text(9),
            peers: Vec::new(),
        };
        assert_eq!(cmd, want);
    }
}

mod awaiting_a_key {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn awaiting_key_stop_right_after_the_key_ends_the_run() {
        // given: a node without a network, waiting for a key
        let settings = Settings::default();
        let ctl = NodeCtl::new_without_network(&settings);
        let identity = IdentityState::Temporary(TemporaryReason::Declined);
        let run = crate::node::output::spawn(run_node_awaiting_key(
            settings,
            identity,
            ctl.clone(),
            test_relaunch(),
        ));

        // when: a key arrives and a stop follows at once
        ctl.switch_network(&key_text(34));
        ctl.stop_on_os_signal();

        // then: the run ends without an error
        let res = timeout(OUTER_LIMIT, run).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "the run: {res:?}");
    }
}

struct Fixture {
    ctl: Arc<NodeCtl>,
}

fn setup() -> Fixture {
    Fixture {
        ctl: NodeCtl::new(&Settings::default()),
    }
}

impl Fixture {
    /// Runs a cycle start that never ends and stops the node shortly after; gives up after `OUTER_LIMIT`.
    async fn when_stopped_during_a_hanging_start(
        &self,
        first_cycle: bool,
    ) -> Result<Option<RunningNode>> {
        let ctl = self.ctl.clone();
        crate::node::output::spawn(async move {
            sleep(STOP_AFTER).await;
            ctl.stop_on_os_signal();
        });
        let start = std::future::pending::<Result<RunningNode>>();
        timeout(OUTER_LIMIT, boot_cycle_with(&self.ctl, first_cycle, start))
            .await
            .expect("the interrupted start must end")
    }

    /// Starts a cycle over a held loopback node and opens the gate after five stop-check periods.
    async fn when_start_finishes_after_five_checks(&self, mark: u8) -> RunningNode {
        let (open, start) = gated_node(mark);
        let boot = boot_cycle_with(&self.ctl, true, start);
        let opener = async move {
            sleep(STOP_POLL * 5).await;
            let _ = open.send(());
        };
        let (res, ()) = timeout(OUTER_LIMIT, async { tokio::join!(boot, opener) })
            .await
            .expect("the start must end");
        res.expect("the start must not fail")
            .expect("a command that does not interrupt must leave the start alone")
    }

    /// Serves the node until the next command and returns that command.
    async fn serve(&self, node: RunningNode) -> Cmd {
        timeout(OUTER_LIMIT, serve_until_command(node, &self.ctl))
            .await
            .expect("the serving loop must end")
    }

    fn then_status_is(&self, want: NodeStatus) {
        assert_eq!(self.ctl.snapshot().node.status, want);
    }
}

fn loopback_config(mark: u8) -> NodeConfig {
    NodeConfig::for_loopback_test(NetworkKey::from_bytes([mark; 32]))
}

/// A loopback node start that begins only after the returned sender fires.
fn gated_node(
    mark: u8,
) -> (
    oneshot::Sender<()>,
    impl Future<Output = Result<RunningNode>> + Send + 'static,
) {
    let (open, gate) = oneshot::channel::<()>();
    let cfg = loopback_config(mark);
    let start = async move {
        let _ = gate.await;
        start_node(cfg).await
    };
    (open, start)
}
