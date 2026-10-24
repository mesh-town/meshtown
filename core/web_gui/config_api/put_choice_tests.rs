//! Tests of the switch choice for the proxy network, for combined changes, unrelated changes and refused writes.

use super::*;

mod proxy_net_choice {
    use super::*;

    mod changed_alone {
        use super::*;

        #[test]
        fn names_that_network() {
            let mut fx = setup(Opts::default());
            let (status, pick) = fx.when_put(json!({"proxy": {"net": OTHER_NET}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::Named(OTHER_NET.to_string()));
        }
    }

    mod set_back_to_main {
        use super::*;

        #[test]
        fn names_main() {
            let mut fx = setup(Opts {
                proxy_net: Some(OTHER_NET),
                ..Opts::default()
            });
            let (status, pick) = fx.when_put(json!({"proxy": {"net": "main"}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::Named("main".to_string()));
        }
    }

    mod sent_unchanged {
        use super::*;

        #[test]
        fn picks_nothing() {
            let mut fx = setup(Opts {
                proxy_net: Some(OTHER_NET),
                ..Opts::default()
            });
            let (status, pick) = fx.when_put(json!({"proxy": {"net": OTHER_NET}}));
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::None);
        }
    }
}

mod exits_and_proxy_net_together {
    use super::*;

    mod exits_become_non_empty {
        use super::*;

        #[test]
        fn this_slot_wins_over_the_named_network() {
            let mut fx = setup(Opts::default());
            let body = json!({"route": {"exits": [exit(1)]}, "proxy": {"net": OTHER_NET}});
            let (status, pick) = fx.when_put(body);
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::ThisSlot);
        }
    }

    mod exits_become_empty {
        use super::*;

        #[test]
        fn the_named_network_is_picked() {
            let mut fx = setup(Opts {
                exits: vec![exit(1)],
                ..Opts::default()
            });
            let body = json!({"route": {"exits": []}, "proxy": {"net": OTHER_NET}});
            let (status, pick) = fx.when_put(body);
            assert_eq!(status, StatusCode::OK);
            assert_eq!(pick, Pick::Named(OTHER_NET.to_string()));
        }
    }
}

mod unrelated_change {
    use super::*;

    #[test]
    fn picks_nothing() {
        let mut fx = setup(Opts {
            exits: vec![exit(1)],
            proxy_net: Some(OTHER_NET),
        });
        let (status, pick) = fx.when_put(json!({"checks": {"interval_sec": 600}}));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            pick,
            Pick::None,
            "neither route.exits nor proxy.net changed"
        );
    }
}

mod refused_write {
    use super::*;

    mod stale_revision_up_front {
        use super::*;

        #[test]
        fn picks_nothing() {
            let mut fx = setup(Opts::default());
            let stale = fx.state.file_report.rev + 5;
            let body = json!({"route": {"exits": [exit(1)]}, "proxy": {"net": OTHER_NET}});
            let (status, pick) = fx.when_put_at(stale, body);
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(pick, Pick::None);
        }
    }

    mod file_changed_during_the_write {
        use super::*;

        #[test]
        fn picks_nothing_although_the_fields_differ() {
            // given: another writer saves the file between the read and the write of this PUT
            let mut fx = setup(Opts::default());
            fx.race_with_a_foreign_write();
            // when
            let body = json!({"route": {"exits": [exit(1)]}, "proxy": {"net": OTHER_NET}});
            let (status, pick) = fx.when_put(body);
            // then: the choice is computed after the write, so a lost write yields none
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(
                pick,
                Pick::None,
                "the choice must not be computed before the write"
            );
        }
    }

    mod invalid_field {
        use super::*;

        #[test]
        fn picks_nothing() {
            let mut fx = setup(Opts::default());
            let body = json!({"route": {"exits": [exit(1)]}, "socks": {"port": 80}});
            let (status, pick) = fx.when_put(body);
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(pick, Pick::None);
        }
    }
}
