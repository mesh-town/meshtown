//! Tests for `NetDirectory`: the shared map of published proxy configurations. A configuration is told apart by
//! its list of chosen exits: every test config carries a different one-peer list.

use std::sync::Arc;

use super::*;
use crate::config::config_file::ProcessSettings;
use crate::exit::PeerConnPool;
use crate::node::RunningNode;
use crate::proxy::front::ProxyFront;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};

const A_TAG: u8 = 2;
const B_TAG: u8 = 3;
const OTHER_TAG: u8 = 9;
const A_ROUTER_TAG: u8 = 21;
const B_ROUTER_TAG: u8 = 22;

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

mod entries {
    use super::*;

    /// given a new directory, then it lists no entry.
    #[test]
    fn of_an_empty_directory_is_empty() {
        let nets = NetDirectory::default();

        assert!(nets.entries().is_empty(), "nothing was put");
    }

    /// given slots put in descending order, then the entries come ascending by slot id.
    #[tokio::test]
    async fn are_sorted_by_slot_id() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_b(), fx.config(B_TAG));
        nets.put(&slot_a(), fx.config(A_TAG));

        let entries = nets.entries();

        assert_eq!(ids_of(&entries), vec![slot_a(), slot_b()]);
        assert_eq!(
            tags_of(&entries),
            vec![vec![peer(A_TAG)], vec![peer(B_TAG)]]
        );
    }

    /// given a slot that was put and one that was removed, then the entries show only the live one.
    #[tokio::test]
    async fn follow_put_and_remove() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_a(), fx.config(A_TAG));
        nets.put(&slot_b(), fx.config(B_TAG));

        nets.remove(&slot_a());

        assert_eq!(ids_of(&nets.entries()), vec![slot_b()]);
    }
}

mod entries_or {
    use super::*;

    /// given an empty directory, then there is one entry: the given config for the main slot.
    #[tokio::test]
    async fn on_an_empty_directory_is_one_main_entry_from_the_argument() {
        let fx = setup().await;
        let nets = NetDirectory::default();

        let entries = nets.entries_or(&fx.config(A_TAG));

        assert_eq!(ids_of(&entries), vec![NetSlotId::main()]);
        assert_eq!(tags_of(&entries), vec![vec![peer(A_TAG)]]);
    }

    /// given a directory with networks, then the entries are the directory's and the argument is ignored.
    #[tokio::test]
    async fn on_a_filled_directory_gives_its_entries_and_ignores_the_argument() {
        let fx = setup().await;
        let nets = NetDirectory::default();
        nets.put(&slot_a(), fx.config(A_TAG));
        nets.put(&slot_b(), fx.config(B_TAG));

        let entries = nets.entries_or(&fx.config(OTHER_TAG));

        assert_eq!(ids_of(&entries), vec![slot_a(), slot_b()]);
        assert_eq!(
            tags_of(&entries),
            vec![vec![peer(A_TAG)], vec![peer(B_TAG)]]
        );
    }
}

mod mesh_addresses {
    use super::*;

    /// given two networks that each name a different device "Router", when the front lists its mesh addresses, then
    /// both are `router-<hex>.meshtown` (the hex is the start of the device id) and the tails differ.
    #[tokio::test]
    async fn of_same_named_devices_in_two_networks_have_different_tails() {
        let front = setup_front_with_router_in_two_networks().await;

        let addresses = front.mesh_addresses();

        assert_eq!(addresses.len(), 2, "one address per device");
        let in_a = addresses.iter().find(|m| m.net == slot_a());
        let in_b = addresses.iter().find(|m| m.net == slot_b());
        let (in_a, in_b) = (
            in_a.expect("an address in net a"),
            in_b.expect("an address in net b"),
        );
        assert_eq!(in_a.peer, peer(A_ROUTER_TAG));
        assert_eq!(in_b.peer, peer(B_ROUTER_TAG));
        let tail_a = then_is_router_address_of(&in_a.host, in_a.peer);
        let tail_b = then_is_router_address_of(&in_b.host, in_b.peer);
        assert_ne!(
            tail_a, tail_b,
            "the two addresses must tell the devices apart"
        );
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

/// The slots of the entries, in the order the directory gave them.
fn ids_of(entries: &[NetEntry]) -> Vec<NetSlotId> {
    entries.iter().map(|entry| entry.id.clone()).collect()
}

/// The exit lists (the marks of the configs) of the entries, in the order the directory gave them.
fn tags_of(entries: &[NetEntry]) -> Vec<Vec<PeerId>> {
    entries
        .iter()
        .map(|entry| entry.cfg.route.chosen_exits())
        .collect()
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

/// A front whose directory holds two networks; in each, a member with its own id is named "Router" and is online.
async fn setup_front_with_router_in_two_networks() -> ProxyFront {
    let settings = ProcessSettings {
        socks_port: 0,
        ..ProcessSettings::default()
    };
    let front = ProxyFront::bind(&settings)
        .await
        .expect("a free loopback port");
    for (slot, tag) in [(slot_a(), A_ROUTER_TAG), (slot_b(), B_ROUTER_TAG)] {
        let fx = setup().await;
        fx.node.members.add_verified_conn(peer(tag));
        fx.node.members.set_name(peer(tag), "Router".to_string());
        front.nets().put(&slot, fx.config(tag));
    }
    front
}

/// The host is `router-<hex>.meshtown` where `<hex>` (4 or more digits) starts the hex id of `device`; returns `<hex>`.
fn then_is_router_address_of(host: &str, device: PeerId) -> String {
    let address = host.strip_suffix(".meshtown").expect("the mesh suffix");
    let tail = address.strip_prefix("router-").expect("label and a tail");
    assert!(tail.len() >= 4, "a tail of at least 4 digits: {host}");
    assert!(
        tail.chars().all(|c| c.is_ascii_hexdigit()),
        "hex digits: {host}"
    );
    assert!(
        device.to_string().starts_with(tail),
        "the tail is the start of the device id: {host}"
    );
    tail.to_string()
}
