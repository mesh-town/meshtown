use super::*;
use crate::config::config_file::NetSlotId;
use crate::proxy::mesh::host::MeshHost;
use crate::transport::net::PeerId;

mod addresses_of_devices {
    use super::*;

    #[test]
    fn unique_label_has_no_tail() {
        let router = id(&[0xab, 0xcd], 1);
        let devs = devices(vec![row(main(), router, "Router"), row(main(), id(&[0x12], 2), "Camera")]);

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
        let (a, b, x) = (id(&[0xbe, 0xef, 0x01], 1), id(&[0x12, 0x34], 2), id(&[0x77], 3));
        let devs = devices(vec![
            row(main(), a, "Router"),
            row(other(), b, "Router"),
            row(main(), x, "router-beef"),
        ]);

        let map = addresses(&devs);

        assert_eq!(map[&x], "router-beef", "the unique label keeps its bare address");
        assert_eq!(map[&a], "router-beef0", "one more hex digit than the colliding label");
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
        let devs = devices(vec![row(main(), a, "Router"), denied(row(other(), b, "Router"))]);

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

        let devs = devices(vec![row(main(), kept, "Kept"), denied(row(main(), dropped, "Dropped"))]);

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

            assert_eq!(devs[0].net, main(), "the online net wins over the smaller one");
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
            let rows = vec![offline(row(main(), peer, "Box")), offline(row(other(), peer, "Box"))];

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

mod lookup_by_host {
    use super::*;

    #[test]
    fn unique_label_is_found_by_the_bare_address() {
        let router = id(&[0xab, 0xcd], 1);
        let devs = devices(vec![row(main(), router, "Router"), row(main(), id(&[0x12], 2), "Camera")]);

        let found = found_peer(lookup(&devs, &host("router")));

        assert_eq!(found, router);
    }

    #[test]
    fn unknown_label_is_not_found() {
        let devs = devices(vec![row(main(), id(&[0xab, 0xcd], 1), "Router")]);

        assert!(is_not_found(lookup(&devs, &host("camera"))));
    }

    #[test]
    fn unique_label_is_also_found_with_a_hex_tail_of_its_id() {
        let router = id(&[0xab, 0xcd], 1);
        let devs = devices(vec![row(main(), router, "Router")]);

        let found = found_peer(lookup(&devs, &host("router-abcd")));

        assert_eq!(found, router);
    }

    mod same_label_in_two_nets {
        use super::*;

        #[test]
        fn bare_label_is_ambiguous_between_both() {
            let fixture = two_routers();

            let candidates = ambiguous_peers(lookup(&fixture.devs, &host("router")));

            assert_eq!(candidates, sorted(vec![fixture.a, fixture.b]));
        }

        #[test]
        fn address_with_the_tail_of_a_finds_a() {
            let fixture = two_routers();

            let found = found_peer(lookup(&fixture.devs, &host("router-abcd")));

            assert_eq!(found, fixture.a);
        }

        #[test]
        fn address_with_the_tail_of_b_finds_b() {
            let fixture = two_routers();

            let found = found_peer(lookup(&fixture.devs, &host("router-1234")));

            assert_eq!(found, fixture.b);
        }

        #[test]
        fn tail_of_three_digits_is_not_found() {
            let fixture = two_routers();

            assert!(is_not_found(lookup(&fixture.devs, &host("router-abc"))));
        }

        #[test]
        fn tail_of_a_foreign_id_is_not_found() {
            let fixture = two_routers();

            assert!(is_not_found(lookup(&fixture.devs, &host("router-ffff"))));
        }

        #[test]
        fn tail_of_the_right_id_under_another_label_is_not_found() {
            let fixture = two_routers();

            assert!(is_not_found(lookup(&fixture.devs, &host("camera-abcd"))));
        }

        #[test]
        fn longer_start_of_the_id_still_finds_the_device() {
            let fixture = two_routers();
            let longer = format!("router-{}", &fixture.a.to_string()[..10]);

            let found = found_peer(lookup(&fixture.devs, &host(&longer)));

            assert_eq!(found, fixture.a);
        }
    }

    mod common_id_start {
        use super::*;

        #[test]
        fn longer_tails_find_their_own_device() {
            let (a, b) = (id(&[0xab, 0xcd, 0x12], 1), id(&[0xab, 0xcd, 0x34], 2));
            let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);

            let found_a = found_peer(lookup(&devs, &host("router-abcd1")));
            let found_b = found_peer(lookup(&devs, &host("router-abcd3")));

            assert_eq!((found_a, found_b), (a, b));
        }

        #[test]
        fn shared_four_digit_tail_is_ambiguous() {
            let (a, b) = (id(&[0xab, 0xcd, 0x12], 1), id(&[0xab, 0xcd, 0x34], 2));
            let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);

            let candidates = ambiguous_peers(lookup(&devs, &host("router-abcd")));

            assert_eq!(candidates, sorted(vec![a, b]));
        }
    }

    mod bad_labels {
        use super::*;

        #[test]
        fn label_with_a_dot_is_not_found() {
            let devs = devices(vec![row(main(), id(&[0xab], 1), "A B")]);

            assert!(is_not_found(lookup(&devs, &host("a.b"))));
        }

        #[test]
        fn empty_label_is_not_found() {
            let devs = devices(vec![row(main(), id(&[0xab], 1), "Router")]);

            assert!(is_not_found(lookup(&devs, &host(""))));
        }

        #[test]
        fn empty_device_list_finds_nothing() {
            assert!(is_not_found(lookup(&[], &host("router"))));
        }
    }
}

mod suggestions {
    use super::*;

    #[test]
    fn near_miss_suggests_the_close_label() {
        let devs = fleet(&["Home OpenWrt", "Kitchen"]);

        assert_eq!(suggest_labels(&devs, "home-openwt"), vec!["home-openwrt"]);
    }

    #[test]
    fn common_start_of_four_characters_suggests_the_longer_labels() {
        let devs = fleet(&["Home OpenWrt", "Home NAS", "Homelab", "Kitchen"]);

        let found = suggest_labels(&devs, "home");

        assert_eq!(found, vec!["homelab", "home-nas", "home-openwrt"]);
    }

    #[test]
    fn nothing_alike_gives_nothing() {
        let devs = fleet(&["Home OpenWrt", "Router", "Box"]);

        assert!(suggest_labels(&devs, "zzz").is_empty());
    }

    #[test]
    fn empty_slug_gives_nothing() {
        let devs = fleet(&["Ab"]);

        assert!(suggest_labels(&devs, "").is_empty());
    }

    #[test]
    fn slug_with_a_dot_gives_nothing() {
        let devs = fleet(&["Home OpenWrt"]);

        assert!(suggest_labels(&devs, "home.openwrt").is_empty());
    }

    #[test]
    fn at_most_three_are_returned() {
        let devs = fleet(&["Home E", "Home D", "Home C", "Home B", "Home A"]);

        let found = suggest_labels(&devs, "home");

        assert_eq!(found, vec!["home-a", "home-b", "home-c"], "equal distance sorts alphabetically");
    }

    #[test]
    fn closer_label_goes_before_a_farther_one() {
        let devs = fleet(&["Boxes", "Boxe"]);

        let found = suggest_labels(&devs, "boxe");

        assert_eq!(found, vec!["boxe", "boxes"]);
    }

    #[test]
    fn equal_distance_is_ordered_alphabetically() {
        let devs = fleet(&["Bog", "Bax"]);

        let found = suggest_labels(&devs, "box");

        assert_eq!(found, vec!["bax", "bog"]);
    }

    #[test]
    fn a_label_worn_by_two_devices_is_suggested_once_without_a_tail() {
        let devs = devices(vec![
            row(main(), id(&[0xab, 0xcd], 1), "Home NAS"),
            row(other(), id(&[0x12, 0x34], 2), "Home NAS"),
        ]);

        let found = suggest_labels(&devs, "home-nsa");

        assert_eq!(found, vec!["home-nas"]);
    }
}

// --- setup and helpers ---

struct Trap {
    devs: Vec<Device>,
    dashed: PeerId,
    plain: PeerId,
}

/// A device named `router-beef` and a device named `router` whose id starts with `beef`.
fn trap() -> Trap {
    let (dashed, plain) = (id(&[0x11], 1), id(&[0xbe, 0xef], 2));
    let devs = devices(vec![row(main(), dashed, "router-beef"), row(main(), plain, "Router")]);
    Trap { devs, dashed, plain }
}

struct TwoRouters {
    devs: Vec<Device>,
    a: PeerId,
    b: PeerId,
}

/// Two devices named `Router` in different nets; ids start with `abcd` and `1234`.
fn two_routers() -> TwoRouters {
    let (a, b) = (id(&[0xab, 0xcd], 1), id(&[0x12, 0x34], 2));
    let devs = devices(vec![row(main(), a, "Router"), row(other(), b, "Router")]);
    TwoRouters { devs, a, b }
}

/// One device per name, each with its own id, all in the main net.
fn fleet(names: &[&str]) -> Vec<Device> {
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, name)| row(main(), id(&[0x80], i as u8), name))
        .collect();
    devices(rows)
}

fn main() -> NetSlotId {
    NetSlotId::main()
}

/// A second net that sorts before `main`.
fn other() -> NetSlotId {
    NetSlotId::parse("aaaaaa").expect("six base32 characters")
}

/// An online, allowed row.
fn row(net: NetSlotId, peer: PeerId, name: &str) -> NameRow {
    NameRow { net, peer, name: name.to_string(), online: true, allowed: true }
}

fn offline(row: NameRow) -> NameRow {
    NameRow { online: false, ..row }
}

fn denied(row: NameRow) -> NameRow {
    NameRow { allowed: false, ..row }
}

fn host(label: &str) -> MeshHost {
    MeshHost { label: label.to_string() }
}

/// A node id on the curve whose hex text starts with `prefix`; `salt` makes ids with the same
/// prefix differ.
fn id(prefix: &[u8], salt: u8) -> PeerId {
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

fn peers_of(devs: &[Device]) -> Vec<PeerId> {
    devs.iter().map(|d| d.peer).collect()
}

fn sorted(mut peers: Vec<PeerId>) -> Vec<PeerId> {
    peers.sort();
    peers
}

fn found_peer(result: Lookup) -> PeerId {
    match result {
        Lookup::Found(device) => device.peer,
        Lookup::Ambiguous(_) => panic!("expected Found, got Ambiguous"),
        Lookup::NotFound => panic!("expected Found, got NotFound"),
    }
}

fn ambiguous_peers(result: Lookup) -> Vec<PeerId> {
    match result {
        Lookup::Ambiguous(devs) => sorted(peers_of(&devs)),
        Lookup::Found(_) => panic!("expected Ambiguous, got Found"),
        Lookup::NotFound => panic!("expected Ambiguous, got NotFound"),
    }
}

fn is_not_found(result: Lookup) -> bool {
    matches!(result, Lookup::NotFound)
}
