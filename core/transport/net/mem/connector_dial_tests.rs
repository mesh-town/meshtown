//! Tests of the `mem` connector (continuation of `connector_tests.rs`): the path after `dial` and a closed connector.

use super::*;

mod dialed_path {
    use super::*;

    #[tokio::test]
    async fn describes_the_target_as_a_direct_mem_path() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));

        let info = dial_ok(&a, &b.local_addr()).await.info();

        assert_eq!(info.peer, peer(2));
        assert!(
            matches!(info.kind, PathKind::Direct),
            "kind = {:?}",
            info.kind
        );
        assert_eq!(info.remote, None);
        assert_eq!(info.family, "mem");
    }

    #[tokio::test]
    async fn the_target_gets_the_other_end_naming_the_dialer() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let _dialed = dial_ok(&a, &b.local_addr()).await;

        let info = listen_path(&b).await.info();

        assert_eq!(info.peer, peer(1));
        assert_eq!(info.family, "mem");
    }

    #[tokio::test]
    async fn carries_packets_both_ways() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let dialed = dial_ok(&a, &b.local_addr()).await;
        let accepted = listen_path(&b).await;

        dialed.send(Bytes::from_static(b"ping")).await.unwrap();
        let ping = recv_ok(&*accepted).await;
        accepted.send(Bytes::from_static(b"pong")).await.unwrap();
        let pong = recv_ok(&*dialed).await;

        assert_eq!((&ping[..], &pong[..]), (&b"ping"[..], &b"pong"[..]));
    }

    #[tokio::test]
    async fn come_out_of_listen_in_the_order_of_dials() {
        let net = MemNet::new();
        let (a, b, c) = (
            net.connector(&secret(1)),
            net.connector(&secret(2)),
            net.connector(&secret(3)),
        );
        let _from_a = dial_ok(&a, &b.local_addr()).await;
        let _from_c = dial_ok(&c, &b.local_addr()).await;

        let first = listen_path(&b).await.info().peer;
        let second = listen_path(&b).await.info().peer;

        assert_eq!((first, second), (peer(1), peer(3)));
    }

    #[tokio::test]
    async fn a_dial_waiting_for_room_in_a_target_that_closes_is_no_route() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let to_b = b.local_addr();
        let mut queued = Vec::new();
        for _ in 0..64 {
            queued.push(dial_ok(&a, &to_b).await); // the accept queue holds 64 paths
        }

        let mut waiting = a.dial(&to_b);
        let early = timeout(Duration::from_millis(100), &mut waiting).await;
        assert!(
            early.is_err(),
            "the 65th dial must wait for room in the target's queue"
        );
        b.close().await;

        let err = match timeout(Duration::from_secs(2), &mut waiting).await {
            Ok(Err(e)) => e,
            Ok(Ok(_)) => panic!("dial into a closed target went through"),
            Err(_) => panic!("dial did not notice that the target closed"),
        };
        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }
}

mod closed_connector {
    use super::*;

    #[tokio::test]
    async fn dial_is_closed() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        a.close().await;

        let err = dial_err(&a, &b.local_addr()).await;

        assert!(
            matches!(err, NetError::Closed),
            "expected Closed, got {err:?}"
        );
    }

    #[tokio::test]
    async fn listen_is_none() {
        let a = MemNet::new().connector(&secret(1));
        a.close().await;

        let got = timeout(Duration::from_secs(2), a.listen()).await;

        assert!(
            matches!(got, Ok(None)),
            "listen of a closed connector must end with None"
        );
    }

    #[tokio::test]
    async fn listen_is_none_even_with_a_path_in_the_queue() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let _queued = dial_ok(&a, &b.local_addr()).await;
        b.close().await;

        let got = timeout(Duration::from_secs(2), b.listen()).await;

        assert!(
            matches!(got, Ok(None)),
            "a queued path must not outlive close"
        );
    }

    #[tokio::test]
    async fn the_dialer_of_a_closed_target_is_no_route() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let to_b = b.local_addr();
        b.close().await;

        let err = dial_err(&a, &to_b).await;

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }
}
