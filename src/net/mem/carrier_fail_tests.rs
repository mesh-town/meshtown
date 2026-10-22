//! Tests of the `mem` carrier (continuation of `carrier_tests.rs`): an invalid first frame and a silent counterpart.

use super::*;

mod bad_first_frame {
    use super::*;

    #[tokio::test]
    async fn an_open_instead_of_hello_fails_the_acceptor() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();
        send_frame(&b, Frame::Open(1)).await;

        let started = start_in_time(&carrier, a, CarrierRole::Accept(list_of(&[AUTH]))).await;

        assert!(started.is_err(), "Open instead of Hello was accepted");
    }

    #[tokio::test]
    async fn a_garbage_first_packet_fails_the_acceptor() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();
        b.send(Bytes::from_static(&[0xFF, 1, 2]))
            .await
            .expect("send of garbage");

        let started = start_in_time(&carrier, a, CarrierRole::Accept(list_of(&[AUTH]))).await;

        assert!(started.is_err(), "a garbage first packet was accepted");
    }

    #[tokio::test]
    async fn an_open_instead_of_accept_fails_the_dialer_with_other() {
        let (a, b) = setup();
        let carrier = MemCarrier::new();

        let (started, ()) = tokio::join!(carrier.start(Box::new(a), CarrierRole::Dial(AUTH)), async {
            recv_frame(&b).await;
            send_frame(&b, Frame::Open(1)).await;
        });

        let err = error_of(started);
        assert!(matches!(err, NetError::Other(_)), "error: {err:?}");
    }
}

mod silence {
    use super::*;

    #[tokio::test]
    async fn a_dialer_facing_a_silent_far_end_times_out() {
        let (a, _silent) = setup();
        let carrier = MemCarrier::with_timeout(Duration::from_millis(100));

        let started = start_in_time(&carrier, a, CarrierRole::Dial(AUTH)).await;

        let err = error_of(started);
        assert!(matches!(err, NetError::Timeout), "error: {err:?}");
    }

    #[tokio::test]
    async fn the_wait_lasts_for_the_set_timeout() {
        let (a, _silent) = setup();
        let carrier = MemCarrier::with_timeout(Duration::from_millis(100));
        let began = Instant::now();

        let _ = start_in_time(&carrier, a, CarrierRole::Dial(AUTH)).await;

        let waited = began.elapsed();
        assert!(
            waited >= Duration::from_millis(90),
            "answered after {waited:?}, the limit is 100 ms"
        );
    }

    #[tokio::test]
    async fn the_path_is_closed_after_a_timeout() {
        let (a, b) = setup();
        let carrier = MemCarrier::with_timeout(Duration::from_millis(100));

        let (started, ()) = tokio::join!(carrier.start(Box::new(a), CarrierRole::Dial(AUTH)), async {
            recv_frame(&b).await;
            assert_eq!(
                recv_err(&b).await,
                ErrorKind::UnexpectedEof,
                "after the time limit the path must be closed"
            );
        });

        assert!(matches!(error_of(started), NetError::Timeout));
    }

    // The time limit applies to `start(Dial)`, and to `start(Accept)` with a silent dialer alike — the same `Timeout` (otherwise a stuck accept holds the task forever).
    #[tokio::test]
    async fn an_acceptor_facing_a_silent_far_end_times_out() {
        let (a, _silent) = setup();
        let carrier = MemCarrier::with_timeout(Duration::from_millis(100));

        let started = start_in_time(&carrier, a, CarrierRole::Accept(list_of(&[AUTH]))).await;

        let err = error_of(started);
        assert!(matches!(err, NetError::Timeout), "error: {err:?}");
    }
}
