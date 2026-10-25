//! Tests for the controller's link to the process proxy: which seat a controller publishes to, under which slot
//! id, and who may reconfigure the process listener. A target is told apart by its list of chosen exits.

use super::*;

use crate::config::config_file::Settings;
use crate::exit::PeerConnPool;
use crate::proxy::route::RouteCtx;
use crate::proxy::ProxyConfig;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};

mod without_a_seat {
    use super::*;

    #[tokio::test]
    async fn node_up_and_node_down_do_not_panic() {
        // given
        let fx = setup().await;
        let ctl = NodeCtl::new(&Settings::default());

        // when
        ctl.node_up(&fx.node);
        ctl.node_down();

        // then
        assert!(ctl.proxy_face().is_none(), "there is no seat, so no face");
        assert!(ctl.proxy_front().is_none(), "there is no seat, so no front");
    }

    #[tokio::test]
    async fn apply_process_proxy_is_ok_and_does_nothing() {
        let ctl = NodeCtl::new(&Settings::default());

        let res = ctl.apply_process_proxy(&settings_on(free_port())).await;

        assert!(res.is_ok(), "got {res:?}");
    }
}

mod set_proxy_seat {
    use super::*;

    #[tokio::test]
    async fn the_second_call_does_not_replace_the_first() {
        // given: a seat is set for the main slot
        let fx = setup().await;
        let second = fx.another_seat().await;
        fx.ctl.set_proxy_seat(NetSlotId::main(), fx.seat.clone());

        // when: another seat is offered for another slot
        fx.ctl.set_proxy_seat(slot_a(), second.clone());
        fx.ctl.node_up(&fx.node);

        // then: the first seat got the node, the second got nothing
        assert!(
            fx.front.current_route().is_some(),
            "the first seat's target"
        );
        assert!(
            second.front().unwrap().current_route().is_none(),
            "the second seat must stay untouched"
        );
        assert_eq!(fx.ctl.proxy_front().unwrap().addr(), fx.front.addr());
    }
}

mod node_up_and_down {
    use super::*;

    #[tokio::test]
    async fn on_the_chosen_slot_node_up_sets_the_target_and_node_down_clears_it() {
        // given
        let fx = setup().await;
        fx.ctl.set_proxy_seat(NetSlotId::main(), fx.seat.clone());

        // when
        fx.ctl.node_up(&fx.node);
        let after_up = fx.target_exits();
        fx.ctl.node_down();
        let after_down = fx.target_exits();

        // then
        assert_eq!(after_up, Some(fx.node.proxy_config().route.chosen_exits()));
        assert_eq!(after_down, None);
    }

    #[tokio::test]
    async fn on_a_foreign_slot_node_up_keeps_the_target() {
        // given: the main slot is chosen and serves the tagged configuration
        let fx = setup().await;
        fx.seat.node_up(&NetSlotId::main(), fx.tagged(7));
        fx.ctl.set_proxy_seat(slot_a(), fx.seat.clone());

        // when
        fx.ctl.node_up(&fx.node);

        // then
        assert_eq!(fx.target_exits(), Some(vec![peer(7)]));
    }

    #[tokio::test]
    async fn on_a_foreign_slot_node_down_keeps_the_target() {
        let fx = setup().await;
        fx.seat.node_up(&NetSlotId::main(), fx.tagged(7));
        fx.ctl.set_proxy_seat(slot_a(), fx.seat.clone());
        fx.ctl.node_up(&fx.node);

        fx.ctl.node_down();

        assert_eq!(fx.target_exits(), Some(vec![peer(7)]));
    }
}

mod apply_process_proxy {
    use super::*;

    #[tokio::test]
    async fn of_the_main_slot_moves_the_front_to_the_new_port() {
        // given
        let fx = setup().await;
        fx.ctl.set_proxy_seat(NetSlotId::main(), fx.seat.clone());
        let port = free_port();

        // when
        let res = fx.ctl.apply_process_proxy(&settings_on(port)).await;

        // then
        assert!(res.is_ok(), "got {res:?}");
        assert_eq!(fx.front.addr().map(|a| a.port()), Some(port));
    }

    #[tokio::test]
    async fn of_a_slot_other_than_main_leaves_the_front_alone() {
        // given
        let fx = setup().await;
        fx.ctl.set_proxy_seat(slot_a(), fx.seat.clone());
        let before = fx.front.addr();

        // when
        let res = fx.ctl.apply_process_proxy(&settings_on(free_port())).await;

        // then
        assert!(res.is_ok(), "got {res:?}");
        assert_eq!(
            fx.front.addr(),
            before,
            "only the main slot owns the listener"
        );
    }

    #[tokio::test]
    async fn of_the_main_slot_without_a_listener_is_ok() {
        // given: a seat without a front
        let fx = setup().await;
        let bare = Arc::new(ProxySeat::new(None, NetSlotId::main()));
        fx.ctl.set_proxy_seat(NetSlotId::main(), bare);

        // when
        let res = fx.ctl.apply_process_proxy(&settings_on(free_port())).await;

        // then
        assert!(res.is_ok(), "got {res:?}");
    }
}

struct Fixture {
    ctl: Arc<NodeCtl>,
    seat: Arc<ProxySeat>,
    front: ProxyFront,
    node: RunningNode,
}

/// A controller, a seat over a front on a free loopback port (main chosen) and a running loopback node.
async fn setup() -> Fixture {
    let front = bind_front().await;
    let seat = Arc::new(ProxySeat::new(Some(front.clone()), NetSlotId::main()));
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    Fixture {
        ctl: NodeCtl::new(&Settings::default()),
        seat,
        front,
        node,
    }
}

async fn bind_front() -> ProxyFront {
    ProxyFront::bind(&settings_on(0))
        .await
        .expect("a free loopback port")
}

impl Fixture {
    async fn another_seat(&self) -> Arc<ProxySeat> {
        Arc::new(ProxySeat::new(Some(bind_front().await), NetSlotId::main()))
    }

    /// The marked exit list of the target the front serves; `None` — no target.
    fn target_exits(&self) -> Option<Vec<PeerId>> {
        self.front.current_route().map(|route| route.chosen_exits())
    }

    /// A proxy config whose single chosen exit is the peer `tag`: the mark that tells it from the node's own.
    fn tagged(&self, tag: u8) -> ProxyConfig {
        let resolver = Resolver::new(
            std::net::SocketAddr::from(([127, 0, 0, 1], 1)),
            HttpLimits::default(),
            "meshtown-test".to_string(),
        )
        .expect("resolver with a stub address");
        ProxyConfig {
            route: RouteCtx::new(
                self.node.members.clone(),
                self.node.domains.clone(),
                self.node.id,
                vec![peer(tag)],
            ),
            stack: self.node.stack.clone(),
            pool: PeerConnPool::default(),
            resolver: Arc::new(resolver),
            host_overrides: Arc::new(Default::default()),
            udp_slots: crate::proxy::udp_slots(),
        }
    }
}

fn settings_on(port: u16) -> ProcessSettings {
    ProcessSettings {
        socks_port: port,
        ..ProcessSettings::default()
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    l.local_addr().unwrap().port()
}

fn peer(tag: u8) -> PeerId {
    NodeSecret::from_bytes(&[tag; 32]).peer_id()
}

fn slot_a() -> NetSlotId {
    NetSlotId::parse("ab2c3d").unwrap()
}
