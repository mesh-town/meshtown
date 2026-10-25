use super::tests::*;
use super::*;

mod lookup_by_host {
    use super::*;

    #[test]
    fn unique_label_is_found_by_the_bare_address() {
        let router = id(&[0xab, 0xcd], 1);
        let devs = devices(vec![
            row(main(), router, "Router"),
            row(main(), id(&[0x12], 2), "Camera"),
        ]);

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

        assert_eq!(
            found,
            vec!["home-a", "home-b", "home-c"],
            "equal distance sorts alphabetically"
        );
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
