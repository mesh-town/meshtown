//! A start that fails after a join from the page or inside a restart does not end the process: the node
//! goes back to waiting for a network and keeps the reason. A failure of the very first start of a run
//! with a key still ends the process.
//!
//! The start fails on the process proxy: the settings name a port that another program holds.

use super::proxy_support::*;
use super::*;
use crate::config::config_file::{CliOverrides, NetSettings, NetSlotId};
use crate::web_gui::{ActionError, NodeControl, NodeStatus};

const STAYS_ALIVE: Duration = Duration::from_secs(2);
const STEP_LIMIT: Duration = Duration::from_secs(20);

mod failed_join {
    use super::*;

    /// given a node waiting for a key and settings with a busy port, when a key is entered, then the node is
    /// offline, the reason names the port and the task goes on waiting.
    #[tokio::test]
    async fn a_failed_join_keeps_the_process_waiting_with_the_reason() {
        let mut fx = setup_join().await;

        fx.ctl.switch_network(&key_text(70));

        fx.then_waits_with_the_busy_port_reason().await;
        fx.then_the_task_keeps_running().await;
    }

    /// given a failed join, when the port is released and the key is entered again, then the node starts and
    /// the old reason is gone.
    #[tokio::test]
    async fn a_join_after_a_failed_one_starts_the_node() {
        let mut fx = setup_join().await;
        fx.ctl.switch_network(&key_text(71));
        fx.then_waits_with_the_busy_port_reason().await;

        fx.when_the_port_is_released();
        fx.ctl.switch_network(&key_text(71));

        wait_until("the node is up", || fx.is_up()).await;
        assert_eq!(fx.reason(), None, "a started node has no start error");
        fx.then_a_stop_ends_the_run().await;
    }

    /// given a failed join, when the node is told to stop, then the run ends without an error.
    #[tokio::test]
    async fn a_stop_after_a_failed_join_ends_the_run() {
        let mut fx = setup_join().await;
        fx.ctl.switch_network(&key_text(72));
        fx.then_waits_with_the_busy_port_reason().await;

        fx.then_a_stop_ends_the_run().await;
    }
}

mod failed_restart {
    use super::*;

    /// given a node that is up and a restart whose settings bring a busy port, when the page restarts it, then
    /// the node is offline with the reason and the task goes on waiting.
    #[tokio::test]
    async fn a_failed_restart_keeps_the_process_waiting_with_the_reason() {
        let mut fx = setup_restart(73).await;
        fx.when_the_node_is_up().await;

        fx.ctl.restart().expect("the node is up");

        fx.then_waits_with_the_busy_port_reason().await;
        fx.then_the_task_keeps_running().await;
    }

    /// given a failed restart, when the node is told to stop, then the run ends without an error.
    #[tokio::test]
    async fn a_stop_after_a_failed_restart_ends_the_run() {
        let mut fx = setup_restart(74).await;
        fx.when_the_node_is_up().await;
        fx.ctl.restart().expect("the node is up");
        fx.then_waits_with_the_busy_port_reason().await;

        fx.then_a_stop_ends_the_run().await;
    }

    /// given a restart and a request to leave the network that land together, when the start fails, then the
    /// network is forgotten and the process goes on waiting for a key.
    #[tokio::test]
    async fn a_leave_net_during_a_failed_restart_forgets_the_network() {
        let mut fx = setup_restart(75).await;
        fx.when_the_node_is_up().await;

        fx.ctl.restart().expect("the node is up");
        fx.ctl
            .request_leave_net(new_secret())
            .expect("a leave while starting");

        wait_until("waiting for a key", || fx.ctl.is_no_network()).await;
        fx.then_the_network_is_forgotten();
        fx.then_the_task_keeps_running().await;
        fx.then_a_stop_ends_the_run().await;
    }
}

mod waiting_for_a_key {
    use super::*;

    /// given a node that waits for the first command and a leave request queued while it was starting,
    /// when the wait loop takes that command, then the network is forgotten and the node waits for a key
    /// (the request is not dropped).
    #[tokio::test]
    async fn a_leave_net_command_in_the_wait_loop_forgets_the_network() {
        let mut fx = setup_starting_with_a_key(76).await;

        fx.ctl
            .request_leave_net(new_secret())
            .expect("a leave while starting");

        wait_until("waiting for a key", || fx.ctl.is_no_network()).await;
        fx.then_the_network_is_forgotten();
        fx.then_the_task_keeps_running().await;
        fx.then_a_stop_ends_the_run().await;
    }
}

mod first_start_of_a_run {
    use super::*;

    /// given a run with a key whose very first start brings a busy port, then the run ends with the bind
    /// error: only later starts fall back to waiting.
    #[tokio::test]
    async fn a_failed_first_start_with_a_key_still_fails_the_run() {
        let fx = setup_first_start(77).await;

        let res = timeout(STEP_LIMIT, fx.task)
            .await
            .expect("the run must end");

        let err = res.expect("the task joins").expect_err("the run fails");
        let bind_error = err
            .downcast_ref::<crate::proxy::BindError>()
            .expect("a BindError");
        assert_eq!(bind_error.port, fx.busy_port);
    }
}

mod start_error_text {
    use super::*;

    fn reason_after_fallback(e: anyhow::Error) -> String {
        let ctl = NodeCtl::new(&Settings::default());
        fall_back_to_waiting(&ctl, &e);
        ctl.start_error().expect("a start error")
    }

    /// given a failure whose message ends with a dot, then the kept reason has no trailing dot and no "..".
    #[tokio::test]
    async fn start_error_drops_trailing_dot() {
        let e = anyhow::Error::new(crate::proxy::BindError {
            port: 1091,
            io_error_kind: std::io::ErrorKind::AddrInUse,
            os_message: "Address already in use (os error 98)".into(),
        });
        assert!(
            e.to_string().ends_with('.'),
            "the source message ends with a dot"
        );

        let reason = reason_after_fallback(e);

        assert!(!reason.ends_with('.'), "{reason}");
        assert!(!reason.contains(".."), "{reason}");
        assert!(reason.contains("1091"), "{reason}");
    }

    /// given a message without a final dot, it stays as is; a dot in the middle stays too.
    #[tokio::test]
    async fn start_error_without_dot_unchanged() {
        assert_eq!(reason_after_fallback(anyhow::anyhow!("no port")), "no port");
        assert_eq!(reason_after_fallback(anyhow::anyhow!("a. b.")), "a. b");
    }
}

/// The run under test: its controller, the port another program holds, and the task.
struct Fx {
    ctl: Arc<NodeCtl>,
    _foreign: Option<std::net::TcpListener>,
    busy_port: u16,
    task: tokio::task::JoinHandle<Result<()>>,
    _front: crate::proxy::ProxyFront,
}

/// A node without a key whose settings name a busy port; the front listens on another free port.
async fn setup_join() -> Fx {
    let (foreign, busy_port) = foreign_listener();
    let settings = Settings {
        process: process_on(busy_port),
        ..Settings::default()
    };
    let ctl = NodeCtl::new_without_network(&settings);
    let front = seat_on_a_free_port(&ctl).await;
    let identity = IdentityState::Temporary(TemporaryReason::Declined);
    let relaunch = Relaunch {
        test_net: Some(crate::transport::net::mem::process_net()),
        ..test_relaunch()
    };
    let task = crate::node::output::spawn(run_node_awaiting_key(
        settings,
        identity,
        ctl.clone(),
        relaunch,
    ));
    Fx::new(ctl, foreign, busy_port, task, front)
}

/// A node with a key, settings on the front's port, and a launch flag that moves the port to a busy one on
/// the next restart (the first start does not read launch flags).
async fn setup_restart(key_byte: u8) -> Fx {
    let (foreign, busy_port) = foreign_listener();
    let front_port = free_port();
    let settings = settings_with_key(key_byte, front_port);
    let ctl = NodeCtl::new(&settings);
    let front = seat_on(&ctl, front_port).await;
    let relaunch = Relaunch {
        cli: CliOverrides {
            socks_port: Some(busy_port),
            ..CliOverrides::default()
        },
        ..test_relaunch()
    };
    let task = run_with_key(&ctl, settings, key_byte, relaunch);
    Fx::new(ctl, foreign, busy_port, task, front)
}

/// A run with a key whose settings name a busy port from the very first start.
async fn setup_first_start(key_byte: u8) -> Fx {
    let (foreign, busy_port) = foreign_listener();
    let settings = settings_with_key(key_byte, busy_port);
    let ctl = NodeCtl::new(&settings);
    let front = seat_on_a_free_port(&ctl).await;
    let task = run_with_key(&ctl, settings, key_byte, test_relaunch());
    Fx::new(ctl, foreign, busy_port, task, front)
}

/// The controller of a node that has a key in the settings and is still `Starting`, in the wait loop.
async fn setup_starting_with_a_key(key_byte: u8) -> Fx {
    let (foreign, busy_port) = foreign_listener();
    let settings = settings_with_key(key_byte, busy_port);
    let ctl = NodeCtl::new(&settings);
    let front = seat_on_a_free_port(&ctl).await;
    let identity = IdentityState::Temporary(TemporaryReason::Declined);
    let task = crate::node::output::spawn(run_node_awaiting_key(
        settings,
        identity,
        ctl.clone(),
        test_relaunch(),
    ));
    Fx::new(ctl, foreign, busy_port, task, front)
}

fn run_with_key(
    ctl: &Arc<NodeCtl>,
    settings: Settings,
    key_byte: u8,
    relaunch: Relaunch,
) -> tokio::task::JoinHandle<Result<()>> {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([key_byte; 32]));
    crate::node::output::spawn(run_node_with_ctl(cfg, settings, ctl.clone(), relaunch))
}

fn settings_with_key(key_byte: u8, socks_port: u16) -> Settings {
    Settings {
        net: NetSettings {
            network_key: Some(key_text(key_byte)),
            ..NetSettings::default()
        },
        process: process_on(socks_port),
    }
}

/// A listener of "another program" and its port.
fn foreign_listener() -> (Option<std::net::TcpListener>, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (Some(listener), port)
}

async fn seat_on_a_free_port(ctl: &NodeCtl) -> crate::proxy::ProxyFront {
    seat_on(ctl, free_port()).await
}

/// A process listener on `port` and a seat for the `main` slot over it.
async fn seat_on(ctl: &NodeCtl, port: u16) -> crate::proxy::ProxyFront {
    let front = crate::proxy::ProxyFront::bind(&process_on(port))
        .await
        .expect("a free loopback port");
    let seat = crate::proxy::ProxySeat::new(Some(front.clone()), NetSlotId::main());
    ctl.set_proxy_seat(NetSlotId::main(), Arc::new(seat));
    front
}

fn new_secret() -> NodeSecret {
    NodeSecret::from_bytes(&[35u8; 32])
}

/// Polls `ready` until it holds; the test fails with `what` after `STEP_LIMIT`.
async fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    let polling = async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    if timeout(STEP_LIMIT, polling).await.is_err() {
        panic!("timed out waiting for: {what}");
    }
}

impl Fx {
    fn new(
        ctl: Arc<NodeCtl>,
        foreign: Option<std::net::TcpListener>,
        busy_port: u16,
        task: tokio::task::JoinHandle<Result<()>>,
        front: crate::proxy::ProxyFront,
    ) -> Fx {
        Fx {
            ctl,
            _foreign: foreign,
            busy_port,
            task,
            _front: front,
        }
    }

    fn status(&self) -> NodeStatus {
        self.ctl.snapshot().node.status
    }

    /// A lone node with no members reports `offline`, so "up" is an attached node with a finished first
    /// collection, not `online`.
    fn is_up(&self) -> bool {
        let node = self.ctl.snapshot().node;
        node.status != NodeStatus::Starting && !node.id.is_empty()
    }

    fn reason(&self) -> Option<String> {
        self.ctl.snapshot().node.start_error
    }

    fn when_the_port_is_released(&mut self) {
        self._foreign = None;
    }

    async fn when_the_node_is_up(&self) {
        wait_until("the node is up", || self.is_up()).await;
    }

    async fn then_waits_with_the_busy_port_reason(&self) {
        wait_until("a start error", || self.reason().is_some()).await;
        let reason = self.reason().unwrap_or_default();
        let port = self.busy_port.to_string();
        assert!(reason.contains(&port), "the reason names {port}: {reason}");
        assert_eq!(self.status(), NodeStatus::Offline);
    }

    async fn then_the_task_keeps_running(&mut self) {
        let early = timeout(STAYS_ALIVE, &mut self.task).await;
        assert!(early.is_err(), "the process must keep waiting: {early:?}");
    }

    async fn then_a_stop_ends_the_run(&mut self) {
        self.ctl.stop();
        let res = timeout(STEP_LIMIT, &mut self.task).await;
        assert!(matches!(res, Ok(Ok(Ok(())))), "stop: {res:?}");
    }

    /// The saved key is gone: a restart has nothing to restart into and says the node has no network.
    fn then_the_network_is_forgotten(&self) {
        let res = self.ctl.restart();
        assert!(
            matches!(res, Err(ActionError::NoNetwork(_))),
            "restart after leaving: {res:?}"
        );
    }
}
