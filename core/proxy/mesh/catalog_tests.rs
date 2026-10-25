use super::*;
use crate::config::config_file::NetSlotId;
use crate::proxy::mesh::host::MeshHost;
use crate::transport::net::PeerId;

mod addresses_of_devices {
    use super::*;

    #[test]
    fn unique_label_has_no_tail() {
        let router = id(&[0xab, 0xcd], 1);
        let devs = devices(vec![
            row(main(), router, "Router"),
            row(main(), id(&[0x12], 2), "Camera"),
        ]);

        let map = addresses(&devs);

        assert_eq!(map[&router], "router");
    }

    #[test]
    fn same_label_in_two_nets_gets_four_hex_tails() {
        let (a, b) = (id(&[0xab, 0xcd], 1), id(&[0x12, 0x34], 2));
        let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);

        let map = addresses(&devs);

        assert_eq!(map[&a], "router-abcd");
        assert_eq!(map[&b], "router-1234");
    }

    #[test]
    fn common_id_start_makes_tails_longer_but_different() {
        let (a, b) = (id(&[0xab, 0xcd, 0x12], 1), id(&[0xab, 0xcd, 0x34], 2));
        let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);

        let map = addresses(&devs);

        assert_eq!(map[&a], "router-abcd1");
        assert_eq!(map[&b], "router-abcd3");
    }

    #[test]
    fn tail_grows_while_it_equals_the_label_of_another_device() {
        let (a, b, x) = (
            id(&[0xbe, 0xef, 0x01], 1),
            id(&[0x12, 0x34], 2),
            id(&[0x77], 3),
        );
        let devs = devices(vec![
            row(main(), a, "Router"),
            row(other(), b, "Router"),
            row(main(), x, "router-beef"),
        ]);

        let map = addresses(&devs);

        assert_eq!(
            map[&x], "router-beef",
            "the unique label keeps its bare address"
        );
        assert_eq!(
            map[&a], "router-beef0",
            "one more hex digit than the colliding label"
        );
        assert_eq!(map[&b], "router-1234");
    }

    #[test]
    fn long_names_stay_within_a_dns_label() {
        let long = "a".repeat(200);
        let (a, b) = (id(&[0xab, 0xcd], 1), id(&[0x12, 0x34], 2));
        let devs = devices(vec![row(main(), a, &long), row(other(), b, &long)]);

        let map = addresses(&devs);

        assert!(map[&a].len() <= 63, "address too long: {}", map[&a]);
        assert!(map[&b].len() <= 63, "address too long: {}", map[&b]);
        assert_ne!(map[&a], map[&b]);
    }

    #[test]
    fn disallowed_twin_does_not_force_a_tail() {
        let (a, b) = (id(&[0xab, 0xcd], 1), id(&[0x12, 0x34], 2));
        let devs = devices(vec![
            row(main(), a, "Router"),
            denied(row(other(), b, "Router")),
        ]);

        let map = addresses(&devs);

        assert_eq!(map[&a], "router");
        assert!(!map.contains_key(&b), "a disallowed device has no address");
    }

    mod address_trap {
        use super::*;

        #[test]
        fn address_of_a_dashed_name_and_of_a_hex_tail_do_not_clash() {
            let fixture = trap();

            let map = addresses(&fixture.devs);

            assert_eq!(map[&fixture.dashed], "router-beef");
            assert_eq!(map[&fixture.plain], "router");
        }

        #[test]
        fn each_issued_address_finds_its_own_device() {
            let fixture = trap();

            let dashed = found_peer(lookup(&fixture.devs, &host("router-beef")));
            let plain = found_peer(lookup(&fixture.devs, &host("router")));

            assert_eq!(dashed, fixture.dashed);
            assert_eq!(plain, fixture.plain);
        }
    }
}

mod device_list {
    use super::*;

    #[test]
    fn label_is_the_address_label_of_the_name() {
        let peer = id(&[0x01], 1);

        let devs = devices(vec![row(main(), peer, "Home OpenWrt")]);

        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].label, "home-openwrt");
        assert_eq!(devs[0].name, "Home OpenWrt");
        assert_eq!(devs[0].peer, peer);
    }

    #[test]
    fn disallowed_row_is_not_listed() {
        let (kept, dropped) = (id(&[0x01], 1), id(&[0x02], 2));

        let devs = devices(vec![
            row(main(), kept, "Kept"),
            denied(row(main(), dropped, "Dropped")),
        ]);

        assert_eq!(peers_of(&devs), vec![kept]);
    }

    mod one_peer_in_two_nets {
        use super::*;

        #[test]
        fn is_one_device() {
            let peer = id(&[0x01], 1);

            let devs = devices(vec![row(main(), peer, "Box"), row(other(), peer, "Box")]);

            assert_eq!(devs.len(), 1);
        }

        #[test]
        fn takes_the_net_where_it_is_online() {
            let peer = id(&[0x01], 1);
            let rows = vec![offline(row(other(), peer, "Box")), row(main(), peer, "Box")];

            let devs = devices(rows);

            assert_eq!(
                devs[0].net,
                main(),
                "the online net wins over the smaller one"
            );
            assert!(devs[0].online);
        }

        #[test]
        fn takes_the_online_net_even_when_it_is_listed_first() {
            let peer = id(&[0x01], 1);
            let rows = vec![row(other(), peer, "Box"), offline(row(main(), peer, "Box"))];

            let devs = devices(rows);

            assert_eq!(devs[0].net, other());
            assert!(devs[0].online);
        }

        #[test]
        fn offline_everywhere_takes_the_smaller_net_id() {
            let peer = id(&[0x01], 1);
            let rows = vec![
                offline(row(main(), peer, "Box")),
                offline(row(other(), peer, "Box")),
            ];

            let devs = devices(rows);

            assert_eq!(devs[0].net, other(), "aaaaaa sorts before main");
            assert!(!devs[0].online);
        }

        #[test]
        fn keeps_the_bare_address_of_its_label() {
            let peer = id(&[0x01], 1);
            let devs = devices(vec![row(main(), peer, "Box"), row(other(), peer, "Box")]);

            let map = addresses(&devs);

            assert_eq!(map[&peer], "box", "the same peer is not a name clash");
        }
    }
}

// --- setup and helpers ---

pub(super) struct Trap {
    pub(super) devs: Vec<Device>,
    pub(super) dashed: PeerId,
    pub(super) plain: PeerId,
}

/// A device named `router-beef` and a device named `router` whose id starts with `beef`.
pub(super) fn trap() -> Trap {
    let (dashed, plain) = (id(&[0x11], 1), id(&[0xbe, 0xef], 2));
    let devs = devices(vec![
        row(main(), dashed, "router-beef"),
        row(main(), plain, "Router"),
    ]);
    Trap {
        devs,
        dashed,
        plain,
    }
}

pub(super) struct TwoRouters {
    pub(super) devs: Vec<Device>,
    pub(super) a: PeerId,
    pub(super) b: PeerId,
}

/// Two devices named `Router` in different nets; ids start with `abcd` and `1234`.
pub(super) fn two_routers() -> TwoRouters {
    let (a, b) = (id(&[0xab, 0xcd], 1), id(&[0x12, 0x34], 2));
    let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);
    TwoRouters { devs, a, b }
}

/// One device per name, each with its own id, all in the main net.
pub(super) fn fleet(names: &[&str]) -> Vec<Device> {
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, name)| row(main(), id(&[0x80], i as u8), name))
        .collect();
    devices(rows)
}

pub(super) fn main() -> NetSlotId {
    NetSlotId::main()
}

/// A second net that sorts before `main`.
pub(super) fn other() -> NetSlotId {
    NetSlotId::parse("aaaaaa").expect("six base32 characters")
}

/// An online, allowed row.
pub(super) fn row(net: NetSlotId, peer: PeerId, name: &str) -> NameRow {
    NameRow {
        net,
        peer,
        name: name.to_string(),
        online: true,
        allowed: true,
    }
}

pub(super) fn offline(row: NameRow) -> NameRow {
    NameRow {
        online: false,
        ..row
    }
}

pub(super) fn denied(row: NameRow) -> NameRow {
    NameRow {
        allowed: false,
        ..row
    }
}

pub(super) fn host(label: &str) -> MeshHost {
    MeshHost {
        label: label.to_string(),
    }
}

/// A node id on the curve whose hex text starts with `prefix`; `salt` makes ids with the same
/// prefix differ.
pub(super) fn id(prefix: &[u8], salt: u8) -> PeerId {
    let mut bytes = [0u8; 32];
    bytes[..prefix.len()].copy_from_slice(prefix);
    bytes[16] = salt;
    (0..=255u8)
        .find_map(|tail| {
            bytes[31] = tail;
            PeerId::from_bytes(&bytes).ok()
        })
        .expect("some last byte gives a point on the curve")
}

pub(super) fn peers_of(devs: &[Device]) -> Vec<PeerId> {
    devs.iter().map(|d| d.peer).collect()
}

pub(super) fn sorted(mut peers: Vec<PeerId>) -> Vec<PeerId> {
    peers.sort();
    peers
}

pub(super) fn found_peer(result: Lookup) -> PeerId {
    match result {
        Lookup::Found(device) => device.peer,
        Lookup::Ambiguous(_) => panic!("expected Found, got Ambiguous"),
        Lookup::NotFound => panic!("expected Found, got NotFound"),
    }
}

pub(super) fn ambiguous_peers(result: Lookup) -> Vec<PeerId> {
    match result {
        Lookup::Ambiguous(devs) => sorted(peers_of(&devs)),
        Lookup::Found(_) => panic!("expected Ambiguous, got Found"),
        Lookup::NotFound => panic!("expected Ambiguous, got NotFound"),
    }
}

pub(super) fn is_not_found(result: Lookup) -> bool {
    matches!(result, Lookup::NotFound)
}
