//! Tests for `ProxySeat`: which network's proxy configuration the single local proxy entry points at. A target
//! is told apart by its list of chosen exits: every test config carries a different one-peer list.

use super::*;

use std::sync::Arc;

use crate::config::config_file::{NetSlotId, ProcessSettings};
use crate::exit::PeerConnPool;
use crate::node::RunningNode;
use crate::proxy::front::{ProxyFace, ProxyFront};
use crate::proxy::route::RouteCtx;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};

const MAIN_TAG: u8 = 1;
const A_TAG: u8 = 2;
const B_TAG: u8 = 3;

mod new {
    use super::*;

    /// given a seat made with a chosen slot, then `chosen()` is that slot and no target is published yet.
    #[tokio::test]
    async fn starts_with_the_given_chosen_slot_and_no_target() {
        // given
        let fx = setup(slot_b()).await;

        // then
        assert_eq!(fx.seat.chosen(), slot_b());
        fx.then_target_is(None);
    }
}

mod node_up {
    use super::*;

    /// given the main slot chosen, when another slot comes up, then the target stays as it was.
    #[tokio::test]
    async fn of_a_foreign_slot_keeps_the_target() {
        // given
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));

        // when
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));

        // then
        fx.then_target_is(Some(MAIN_TAG));
    }

    /// given nothing published, when a foreign slot comes up, then there is still no target.
    #[tokio::test]
    async fn of_a_foreign_slot_does_not_publish_when_there_was_no_target() {
        let fx = setup(NetSlotId::main()).await;

        fx.seat.node_up(&slot_a(), fx.config(A_TAG));

        fx.then_target_is(None);
    }

    /// given the main slot chosen, when the main slot comes up, then its configuration becomes the target.
    #[tokio::test]
    async fn of_the_chosen_slot_publishes_its_config() {
        let fx = setup(NetSlotId::main()).await;

        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));

        fx.then_target_is(Some(MAIN_TAG));
    }

    /// given the chosen slot up, when it comes up again with a new config (a restart), then the new one is the target.
    #[tokio::test]
    async fn of_the_chosen_slot_again_replaces_the_target() {
        let fx = setup(slot_a()).await;
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));

        fx.seat.node_up(&slot_a(), fx.config(B_TAG));

        fx.then_target_is(Some(B_TAG));
    }
}

mod node_down {
    use super::*;

    /// given the chosen slot up, when it goes down, then the proxy has no target and refuses connections.
    #[tokio::test]
    async fn of_the_chosen_slot_clears_the_target() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));

        fx.seat.node_down(&NetSlotId::main());

        fx.then_target_is(None);
    }

    /// given the chosen slot up, when another slot goes down, then the target stays.
    #[tokio::test]
    async fn of_a_foreign_slot_keeps_the_target() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));

        fx.seat.node_down(&slot_a());

        fx.then_target_is(Some(MAIN_TAG));
    }

    /// given a slot that never came up, when it goes down, then nothing changes and nothing panics.
    #[tokio::test]
    async fn of_a_slot_that_never_came_up_is_harmless() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));

        fx.seat.node_down(&slot_b());

        fx.then_target_is(Some(MAIN_TAG));
    }

    /// given the chosen slot went down and came up again, then the proxy has the target again.
    #[tokio::test]
    async fn then_up_again_restores_the_target() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        fx.seat.node_down(&NetSlotId::main());

        fx.seat.node_up(&NetSlotId::main(), fx.config(B_TAG));

        fx.then_target_is(Some(B_TAG));
    }
}

mod choose {
    use super::*;

    /// given slot A is up and the main slot is chosen, when A is chosen, then A's configuration is published.
    #[tokio::test]
    async fn publishes_the_config_of_a_slot_that_came_up_earlier() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));

        fx.seat.choose(slot_a());

        fx.then_target_is(Some(A_TAG));
        assert_eq!(fx.seat.chosen(), slot_a());
    }

    /// given slot B is down, when B is chosen, then there is no target (a direct path would leak), and
    /// when B comes up afterwards, then its configuration is the target.
    #[tokio::test]
    async fn a_slot_that_is_down_gives_no_target_until_it_comes_up() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));

        fx.seat.choose(slot_b());
        fx.then_target_is(None);
        fx.seat.node_up(&slot_b(), fx.config(B_TAG));

        fx.then_target_is(Some(B_TAG));
    }

    /// given A was chosen and went down, when the main slot is chosen again, then the main configuration is the target.
    #[tokio::test]
    async fn back_to_the_main_slot_publishes_its_config() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));
        fx.seat.choose(slot_a());

        fx.seat.choose(NetSlotId::main());

        fx.then_target_is(Some(MAIN_TAG));
        assert_eq!(fx.seat.chosen(), NetSlotId::main());
    }

    /// given A is chosen, when the previously chosen main slot comes up or goes down, then the target stays A's.
    #[tokio::test]
    async fn makes_the_previous_slot_foreign() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));
        fx.seat.choose(slot_a());

        fx.seat.node_down(&NetSlotId::main());
        fx.then_target_is(Some(A_TAG));
        fx.seat.node_up(&NetSlotId::main(), fx.config(B_TAG));

        fx.then_target_is(Some(A_TAG));
    }

    /// given slot A went down while it was not chosen, when A is chosen, then there is no target: a stale
    /// configuration of a stopped node is not used.
    #[tokio::test]
    async fn a_slot_that_went_down_while_foreign_gives_no_target() {
        let fx = setup(NetSlotId::main()).await;
        fx.seat.node_up(&slot_a(), fx.config(A_TAG));
        fx.seat.node_down(&slot_a());

        fx.seat.choose(slot_a());

        fx.then_target_is(None);
    }
}

mod chosen {
    use super::*;

    /// given several choices in a row, then `chosen()` reports the last one.
    #[tokio::test]
    async fn reports_the_last_choice() {
        let fx = setup(NetSlotId::main()).await;

        fx.seat.choose(slot_a());
        fx.seat.choose(slot_b());

        assert_eq!(fx.seat.chosen(), slot_b());
    }
}

mod without_a_front {
    use super::*;

    /// given a seat without a front, then its face is the constant `OFF`.
    #[test]
    fn face_is_off() {
        let seat = ProxySeat::new(None, NetSlotId::main());

        let face = seat.face();

        assert_eq!(*face.borrow(), ProxyFace::OFF);
    }

    /// given a seat without a front, then `front()` is `None`.
    #[test]
    fn has_no_front() {
        let seat = ProxySeat::new(None, NetSlotId::main());

        assert!(seat.front().is_none(), "no front was given");
    }

    /// given a seat without a front, when slots go up, go down and are chosen, then it does not panic and tracks `chosen()`.
    #[tokio::test]
    async fn events_are_accepted_and_the_choice_is_tracked() {
        let fx = setup(NetSlotId::main()).await;
        let seat = ProxySeat::new(None, NetSlotId::main());

        seat.node_up(&NetSlotId::main(), fx.config(MAIN_TAG));
        seat.choose(slot_a());
        seat.node_up(&slot_a(), fx.config(A_TAG));
        seat.node_down(&slot_a());

        assert_eq!(seat.chosen(), slot_a());
        assert_eq!(*seat.face().borrow(), ProxyFace::OFF);
    }
}

mod with_a_front {
    use super::*;

    /// given a seat with a front, then `face()` follows the front's listening address.
    #[tokio::test]
    async fn face_is_the_front_face() {
        let fx = setup(NetSlotId::main()).await;

        let face = fx.seat.face();

        let addr = fx.front.addr();
        assert!(addr.is_some(), "the front listens on a free port");
        assert_eq!(face.borrow().addr, addr);
    }

    /// given a seat with a front, then `front()` is that front.
    #[tokio::test]
    async fn front_is_the_given_one() {
        let fx = setup(NetSlotId::main()).await;

        let front = fx.seat.front().expect("the seat was made with a front");

        assert_eq!(front.addr(), fx.front.addr());
    }
}

struct Fixture {
    seat: ProxySeat,
    front: ProxyFront,
    node: RunningNode,
}

/// A seat over a front on a free loopback port; `chosen` is the slot chosen at the start.
async fn setup(chosen: NetSlotId) -> Fixture {
    let settings = ProcessSettings {
        socks_port: 0,
        ..ProcessSettings::default()
    };
    let front = ProxyFront::bind(&settings)
        .await
        .expect("a free loopback port");
    let seat = ProxySeat::new(Some(front.clone()), chosen);
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    Fixture { seat, front, node }
}

impl Fixture {
    /// A proxy config whose chosen exits are the single peer `tag`: the mark that tells configs apart.
    fn config(&self, tag: u8) -> ProxyConfig {
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

    /// The mark of the configuration the front currently serves; `None` — the front has no target.
    fn then_target_is(&self, tag: Option<u8>) {
        let want = tag.map(|tag| vec![peer(tag)]);
        let got = self.front.current_route().map(|route| route.chosen_exits());
        assert_eq!(got, want, "the front's target (marked by its exit list)");
    }
}

fn peer(tag: u8) -> PeerId {
    NodeSecret::from_bytes(&[tag; 32]).peer_id()
}

fn slot_a() -> NetSlotId {
    NetSlotId::parse("ab2c3d").unwrap()
}

fn slot_b() -> NetSlotId {
    NetSlotId::parse("xy4z56").unwrap()
}
