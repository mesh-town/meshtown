//! Tests for `SplitTransport` (continuation of `split_tests.rs`): `connect`, `close`, delegation to the connector.

use super::*;

mod connect {
    use super::*;

    #[tokio::test]
    async fn starts_the_carrier_as_the_dialer_for_the_service() {
        let fx = setup(Opts::default());

        let conn = fx
            .when_connecting(SVC)
            .await
            .expect("connect must succeed");

        assert_eq!(conn.peer(), peer(2));
        let started = fx.carrier.started();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].dialed, Some(SVC));
        assert_eq!(
            started[0].peer,
            peer(2),
            "the carrier received the path to the peer"
        );
    }

    #[tokio::test]
    async fn a_carrier_that_cannot_carry_the_path_is_no_route() {
        let fx = setup(Opts {
            can_carry: false,
            ..Opts::default()
        });

        let err = fx
            .when_connecting(SVC)
            .await
            .err()
            .expect("connect must refuse");

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
        assert!(
            fx.carrier.started().is_empty(),
            "the carrier must not start on a foreign path"
        );
    }

    #[tokio::test]
    async fn the_path_the_carrier_refused_is_closed_for_the_far_side() {
        let fx = setup(Opts {
            can_carry: false,
            ..Opts::default()
        });
        let _ = fx.when_connecting(SVC).await;

        let far_end = fx.then_remote_listens().await;

        let got = timeout(Duration::from_secs(2), far_end.recv()).await;
        let err = got.expect("recv hung: the path is not closed").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn an_unreachable_address_comes_up_as_no_route() {
        let fx = setup(Opts::default());
        let ip_only = PeerAddr {
            id: peer(2),
            hints: vec![AddrHint::Ip("127.0.0.1:9".parse().unwrap())],
        };

        let res = timeout(Duration::from_secs(2), fx.transport.connect(&ip_only, SVC)).await;

        let err = res
            .expect("connect hung")
            .err()
            .expect("connect must refuse");
        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_foreign_key_comes_up_as_wrong_peer() {
        let fx = setup(Opts::default());
        let wrong = PeerAddr {
            id: peer(3),
            hints: fx.remote.local_addr().hints,
        };

        let res = timeout(Duration::from_secs(2), fx.transport.connect(&wrong, SVC)).await;

        let err = res
            .expect("connect hung")
            .err()
            .expect("connect must refuse");
        let NetError::WrongPeer { want, got } = err else {
            panic!("expected WrongPeer, got {err:?}");
        };
        assert_eq!((want, got), (peer(3), peer(2)));
    }
}

mod close {
    use super::*;

    #[tokio::test]
    async fn accept_ends_with_none() {
        let fx = setup(Opts {
            serve: Some(vec![SVC]),
            ..Opts::default()
        });

        fx.transport.close().await;

        assert!(
            fx.accepted().await.is_none(),
            "after close, accept must return None"
        );
    }

    #[tokio::test]
    async fn an_accept_already_waiting_ends_with_none() {
        let fx = setup(Opts {
            serve: Some(vec![SVC]),
            ..Opts::default()
        });

        let (got, ()) = tokio::join!(
            timeout(Duration::from_secs(2), fx.transport.accept()),
            async {
                sleep(Duration::from_millis(50)).await;
                fx.transport.close().await;
            }
        );

        let got = got.expect("the pending accept did not wake up on close");
        assert!(got.is_none(), "the pending accept must return None");
    }

    #[tokio::test]
    async fn connect_is_closed() {
        let fx = setup(Opts::default());
        fx.transport.close().await;

        let err = fx
            .when_connecting(SVC)
            .await
            .err()
            .expect("connect must refuse");

        assert!(
            matches!(err, NetError::Closed),
            "expected Closed, got {err:?}"
        );
    }

    #[tokio::test]
    async fn the_connector_stops_listening() {
        let fx = setup(Opts::default());

        fx.transport.close().await;

        let got = timeout(Duration::from_secs(2), fx.connector.listen()).await;
        assert!(
            matches!(got, Ok(None)),
            "the connector's listen after close must end with None"
        );
    }

    #[tokio::test]
    async fn finishes_even_while_a_start_is_stuck() {
        let fx = setup(Opts {
            steps: vec![Step::Hang],
            serve: Some(vec![SVC]),
            ..Opts::default()
        });
        let _dialed = fx.when_remote_dials().await;
        fx.then_carrier_started(1).await;

        let done = timeout(Duration::from_secs(2), fx.transport.close()).await;

        assert!(done.is_ok(), "close did not wait for the end of the accept loop");
    }
}

mod delegation_to_the_connector {
    use super::*;

    #[tokio::test]
    async fn kind_is_the_connectors() {
        let fx = setup(Opts::default());

        assert_eq!(fx.transport.kind(), "mem");
    }

    #[tokio::test]
    async fn me_is_the_given_id() {
        let fx = setup(Opts::default());

        assert_eq!(fx.transport.me(), peer(1));
    }

    #[tokio::test]
    async fn local_addr_is_the_connectors() {
        let fx = setup(Opts::default());

        assert_eq!(fx.transport.local_addr(), fx.connector.local_addr());
    }

    #[tokio::test]
    async fn can_reach_follows_the_connector() {
        let fx = setup(Opts::default());
        let ip_only = PeerAddr {
            id: peer(2),
            hints: vec![AddrHint::Ip("127.0.0.1:9".parse().unwrap())],
        };

        assert!(fx.transport.can_reach(&fx.remote.local_addr()));
        assert!(!fx.transport.can_reach(&ip_only));
    }

    #[tokio::test]
    async fn net_info_is_the_connectors() {
        let fx = setup(Opts::default());

        let got = format!("{:?}", fx.transport.net_info());

        assert_eq!(got, format!("{:?}", NetInfo::default()));
    }

    // The `debug_assert!(me == local_addr().id)` is assumed to fire at construction (`new`), not only when `me()`/`local_addr()` is called.
    #[cfg(debug_assertions)]
    #[tokio::test]
    #[should_panic]
    async fn a_me_that_is_not_the_connectors_id_is_a_bug() {
        let connector = MemNet::new().connector(&secret(1));

        let t = SplitTransport::new(peer(9), connector, StubCarrier::new(true, vec![]));

        let _ = (t.me(), t.local_addr());
    }
}
