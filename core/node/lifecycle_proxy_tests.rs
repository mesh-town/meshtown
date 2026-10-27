//! Process-proxy cycle tests: the cycle loop publishes to a listener the node does not own.

use super::proxy_support::*;
use super::*;

mod process_proxy {
    use super::*;

    mod node_up {
        use super::*;

        /// given a listener without a target, when a cycle starts, then the listener's target is that node's path.
        #[tokio::test]
        async fn after_the_start_the_front_serves_the_node_path() {
            let fx = setup(51).await;
            fx.then_target_is(None);

            let node = fx.when_cycle_starts(fx.port, 1).await;

            fx.then_target_is(Some(1));
            fx.when_cycle_is_stopped(node).await;
        }
    }

    mod node_down {
        use super::*;

        /// given a node that serves the listener and a shutdown held at its last step, when the node is
        /// told to stop, then the target is gone while the shutdown is still running: new connections are
        /// refused, not sent into a closing node.
        #[tokio::test]
        async fn the_front_has_no_target_before_the_shutdown_ends() {
            let fx = setup(52).await;
            let mut node = fx.when_cycle_starts(fx.port, 1).await;
            let held = hold_replica_flush(&mut node).await;
            fx.ctl.stop_on_os_signal();
            let mut ending = Box::pin(serve_until_command(node, &fx.ctl));

            let first_poll = timeout(Duration::ZERO, &mut ending).await;

            assert!(first_poll.is_err(), "the shutdown must still be running");
            fx.then_target_is(None);
            held.close();
            assert!(matches!(ending.await, Cmd::Stop));
        }

        #[tokio::test]
        async fn the_front_stays_without_a_target_after_the_stop() {
            let fx = setup(53).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;

            fx.when_cycle_is_stopped(node).await;

            fx.then_target_is(None);
        }
    }

    mod restart_with_a_new_socks_port {
        use super::*;

        /// given a node up on port A, when the restart brings settings with port B, then the front listens on B,
        /// A is free again, and the new node's path is the target.
        #[tokio::test]
        async fn the_front_moves_to_the_new_port_and_serves_the_new_node() {
            let fx = setup(54).await;
            let (old_port, new_port) = (fx.port, free_port());
            let node = fx.when_cycle_starts(old_port, 1).await;
            let cmd = fx.when_cycle_is_restarted(node).await;
            assert!(matches!(cmd, Cmd::Switch { .. }), "restart: {cmd:?}");

            let node = fx.when_cycle_starts(new_port, 2).await;

            assert_eq!(fx.front_port(), Some(new_port));
            assert!(can_bind(old_port), "the old port must be released");
            assert!(!can_bind(new_port), "the front must hold the new port");
            fx.then_target_is(Some(2));
            fx.when_cycle_is_stopped(node).await;
        }

        #[tokio::test]
        async fn the_same_port_keeps_the_front_where_it_was() {
            let fx = setup(55).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;
            fx.when_cycle_is_restarted(node).await;

            let node = fx.when_cycle_starts(fx.port, 2).await;

            assert_eq!(fx.front_port(), Some(fx.port));
            fx.then_target_is(Some(2));
            fx.when_cycle_is_stopped(node).await;
        }
    }

    mod new_port_is_taken_by_another_program {
        use super::*;

        /// given a node up, when the next start brings a busy port, then the start fails with the bind error,
        /// the old port still listens and the front has no target (the node it served is not the one being started).
        #[tokio::test]
        async fn the_start_fails_and_the_front_keeps_its_port_without_a_target() {
            let fx = setup(56).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;
            let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let busy = foreign.local_addr().unwrap().port();

            let err = fx.when_cycle_start_fails(busy, 2).await;

            let bind_error = err
                .downcast_ref::<crate::proxy::BindError>()
                .expect("a BindError");
            assert_eq!(bind_error.port, busy);
            assert_eq!(
                fx.front_port(),
                Some(fx.port),
                "the old listener keeps working"
            );
            fx.then_target_is(None);
            node.shutdown().await;
        }
    }
}

/// A process listener on a free port, a seat for the `main` slot over it and a controller that publishes there.
struct Cycle {
    ctl: Arc<NodeCtl>,
    front: crate::proxy::ProxyFront,
    port: u16,
    key_byte: u8,
}

/// `key_byte` makes the network of the test's nodes its own: tests run side by side in one in-memory network.
async fn setup(key_byte: u8) -> Cycle {
    let port = free_port();
    let front = crate::proxy::ProxyFront::bind(&process_on(port))
        .await
        .expect("a free loopback port");
    let seat = crate::proxy::ProxySeat::new(
        Some(front.clone()),
        crate::config::config_file::NetSlotId::main(),
    );
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            network_key: Some(key_text(key_byte)),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let ctl = NodeCtl::new(&settings);
    ctl.set_proxy_seat(
        crate::config::config_file::NetSlotId::main(),
        Arc::new(seat),
    );
    Cycle {
        ctl,
        front,
        port,
        key_byte,
    }
}

impl Cycle {
    /// The node of this cycle; its chosen exit list is `exit_mark(mark)`, which tells nodes apart.
    fn node_config(&self, mark: u8) -> NodeConfig {
        let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([self.key_byte; 32]));
        cfg.settings.route_exits = vec![exit_mark(mark)];
        cfg.proxy_face = self.ctl.proxy_face();
        cfg
    }

    async fn when_cycle_starts(&self, socks_port: u16, mark: u8) -> RunningNode {
        self.start_cycle(socks_port, mark)
            .await
            .expect("the cycle starts")
    }

    async fn when_cycle_start_fails(&self, socks_port: u16, mark: u8) -> anyhow::Error {
        let started = self.start_cycle(socks_port, mark).await;
        started.err().expect("the cycle must not start")
    }

    /// The start of a cycle the way `run_cycles` does it: the process proxy is brought to the settings
    /// the cycle comes up with, then the node boots.
    async fn start_cycle(&self, socks_port: u16, mark: u8) -> Result<RunningNode> {
        apply_proxy(&settings_on(socks_port), &self.ctl).await?;
        let started = boot_cycle(&self.node_config(mark), &self.ctl, false).await?;
        Ok(started.expect("no stop was requested"))
    }

    async fn when_cycle_is_stopped(&self, node: RunningNode) {
        self.ctl.stop_on_os_signal();
        assert!(matches!(
            serve_until_command(node, &self.ctl).await,
            Cmd::Stop
        ));
    }

    /// The page's "restart" button: the node stops with a `Switch` to the same key.
    async fn when_cycle_is_restarted(&self, node: RunningNode) -> Cmd {
        self.ctl.restart().expect("the node is up");
        serve_until_command(node, &self.ctl).await
    }

    fn front_port(&self) -> Option<u16> {
        self.front.addr().map(|addr| addr.port())
    }

    /// The target of the listener is told by the node's exit list; `None` — no target.
    fn then_target_is(&self, mark: Option<u8>) {
        let want = mark.map(|mark| vec![exit_mark(mark)]);
        let got = self.front.current_route().map(|route| route.chosen_exits());
        assert_eq!(got, want, "the listener's target (marked by its exit list)");
    }
}
