use super::*;

const A: LocalKey = 1;
const B: LocalKey = 2;

fn table() -> (SessionTable, Instant) {
    (SessionTable::new(LANE_SESSIONS), Instant::now())
}

mod issuing {
    use super::*;

    #[test]
    fn same_local_id_of_two_locals_gets_two_shared_ids() {
        let (mut t, now) = table();
        let a = t.bind(A, 1, now).expect("a");
        let b = t.bind(B, 1, now).expect("b");
        assert_ne!(a, b);
        assert_eq!(t.route_reply(a, now), Some((A, 1)));
        assert_eq!(t.route_reply(b, now), Some((B, 1)));
    }

    #[test]
    fn bind_twice_returns_the_same_id() {
        let (mut t, now) = table();
        let first = t.bind(A, 5, now);
        assert_eq!(t.bind(A, 5, now), first);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn zero_is_never_issued() {
        let (mut t, now) = table();
        t.next = u16::MAX;
        assert_eq!(t.bind(A, 1, now), Some(u16::MAX));
        assert_eq!(t.bind(A, 2, now), Some(1));
        for i in 3..200 {
            assert_ne!(t.bind(A, i, now), Some(0));
        }
    }

    #[test]
    fn local_id_zero_is_bound() {
        let (mut t, now) = table();
        let shared = t.bind(A, 0, now).expect("bound");
        assert_eq!(t.route_reply(shared, now), Some((A, 0)));
    }

    #[test]
    fn released_id_is_not_reissued_before_wrap() {
        let (mut t, now) = table();
        let released = t.bind(A, 1, now).expect("bound");
        t.unbind(A, 1);
        for i in 0..1000u16 {
            assert_ne!(t.bind(B, i, now), Some(released));
            t.unbind(B, i);
        }
    }

    #[test]
    fn wrap_skips_ids_in_use() {
        let (mut t, now) = table();
        assert_eq!(t.bind(A, 1, now), Some(1));
        t.next = u16::MAX;
        assert_eq!(t.bind(B, 1, now), Some(u16::MAX));
        assert_eq!(t.bind(B, 2, now), Some(2));
    }
}

mod replies {
    use super::*;

    #[test]
    fn unknown_shared_id_routes_nowhere() {
        let (mut t, now) = table();
        assert_eq!(t.route_reply(77, now), None);
    }

    #[test]
    fn reply_refreshes_the_binding() {
        let (mut t, now) = table();
        let shared = t.bind(A, 1, now).expect("bound");
        t.route_reply(shared, now + Duration::from_secs(50));
        assert!(t
            .sweep(now + Duration::from_secs(70), SESSION_IDLE)
            .is_empty());
    }
}

mod release {
    use super::*;

    #[test]
    fn unbind_local_returns_every_shared_id_of_the_local() {
        let (mut t, now) = table();
        let mut own: Vec<u16> = (1..=3).filter_map(|i| t.bind(A, i, now)).collect();
        let other = t.bind(B, 1, now).expect("other");
        let mut freed = t.unbind_local(A);
        own.sort_unstable();
        freed.sort_unstable();
        assert_eq!(freed, own);
        assert_eq!(t.len(), 1);
        assert_eq!(t.route_reply(other, now), Some((B, 1)));
    }

    #[test]
    fn sweep_releases_only_idle_bindings() {
        let (mut t, now) = table();
        let old = t.bind(A, 1, now).expect("old");
        let fresh = t.bind(A, 2, now + Duration::from_secs(50)).expect("fresh");
        let swept = t.sweep(now + Duration::from_secs(61), SESSION_IDLE);
        assert_eq!(swept, vec![old]);
        assert_eq!(t.route_reply(fresh, now), Some((A, 2)));
    }
}

mod limits {
    use super::*;

    #[test]
    fn full_table_refuses_a_new_binding() {
        let (mut t, now) = table();
        let first = t.bind(A, 0, now);
        for i in 1..LANE_SESSIONS as u16 {
            assert!(t.bind(A, i, now).is_some());
        }
        assert_eq!(t.bind(B, 1, now), None);
        assert_eq!(t.bind(A, 0, now), first);
    }
}
