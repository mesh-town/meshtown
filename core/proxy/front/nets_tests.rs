//! Tests for `NetDirectory`: the shared map of published proxy configurations. A configuration is told apart by
//! its list of chosen exits: every test config carries a different one-peer list.

use std::sync::Arc;

use super::*;
use crate::exit::PeerConnPool;
use crate::node::RunningNode;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};

const A_TAG: u8 = 2;
const B_TAG: u8 = 3;

mod empty_directory {
    use super::*;

    /// given a new directory, then it knows no network.
    #[test]
    fn knows_no_network() {
        let nets = NetDirectory::default();

        assert!(nets.get(&slot_a()).is_none(), "nothing was put");
    }
}

mod put {
    use super::*;

    /// given an empty directory, when a config is put for a slot, then `get` returns that config.
    #[tokio::test]
    async fn makes_the_config_available_by_slot() {
        let fx = setup().await;
        let nets = NetDirectory::default();

        nets.put(&slot_a(), fx.config(A_TAG));

        then_tag_is(&nets, &slot_a(), Some(A_TAG));
    }

    /// given a slot with a config, when another config is put for it, then the new one replaces the old.
    #[tokio::test]
    async fn for_a_known_slot_replaces_the_config() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_a(), fx.config(A_TAG));

        nets.put(&slot_a(), fx.config(B_TAG));

        then_tag_is(&nets, &slot_a(), Some(B_TAG));
    }
}

mod remove {
    use super::*;

    /// given two slots, when one is removed, then only the other one is still there.
    #[tokio::test]
    async fn drops_only_the_given_slot() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_a(), fx.config(A_TAG));
        nets.put(&slot_b(), fx.config(B_TAG));

        nets.remove(&slot_a());

        then_tag_is(&nets, &slot_a(), None);
        then_tag_is(&nets, &slot_b(), Some(B_TAG));
    }

    /// given a slot that was never put, when it is removed, then nothing changes and nothing panics.
    #[tokio::test]
    async fn of_an_unknown_slot_is_harmless() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_a(), fx.config(A_TAG));

        nets.remove(&slot_b());

        then_tag_is(&nets, &slot_a(), Some(A_TAG));
    }
}

mod clone {
    use super::*;

    /// given a clone of the directory, when the original changes, then the clone sees the same records.
    #[tokio::test]
    async fn shares_the_records_with_the_original() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        let copy = nets.clone();

        nets.put(&slot_a(), fx.config(A_TAG));
        nets.put(&slot_b(), fx.config(B_TAG));
        nets.remove(&slot_b());

        then_tag_is(&copy, &slot_a(), Some(A_TAG));
        then_tag_is(&copy, &slot_b(), None);
    }
}

struct Fixture {
    node: RunningNode,
}

/// A real node on loopback: the source of the parts of a proxy config.
async fn setup() -> Fixture {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    Fixture { node }
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
            route: crate::proxy::route::RouteCtx::new(
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

/// The directory holds, for `slot`, the config marked `tag`; `None` — it holds nothing for the slot.
fn then_tag_is(nets: &NetDirectory, slot: &NetSlotId, tag: Option<u8>) {
    let want = tag.map(|tag| vec![peer(tag)]);
    let got = nets.get(slot).map(|cfg| cfg.route.chosen_exits());
    assert_eq!(
        got, want,
        "the config of the slot (marked by its exit list)"
    );
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
