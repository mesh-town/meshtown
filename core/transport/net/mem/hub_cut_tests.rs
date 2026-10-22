//! Tests of the `mem` hub: breaking (`cut`) and lifting the break (`heal`) between nodes (continuation of `hub_tests.rs`).

use super::*;

mod cut {
    use super::*;

    #[tokio::test]
    async fn the_dialing_end_reads_eof() {
        let t = setup_triangle().await;

        t.cut_b_a();

        assert_eq!(recv_err(&*t.a_to_b).await, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn the_listening_end_reads_eof() {
        let t = setup_triangle().await;

        t.cut_b_a();

        assert_eq!(recv_err(&*t.b_to_a).await, ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn a_recv_already_waiting_wakes_with_eof() {
        let t = setup_triangle().await;

        let (got, _) = tokio::join!(timeout(Duration::from_secs(2), t.b_to_a.recv()), async {
            sleep(Duration::from_millis(50)).await;
            t.cut_b_a();
        });

        let err = got
            .expect("the waiting recv did not wake up on cut")
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn a_new_dial_from_a_to_b_is_no_route() {
        let t = setup_triangle().await;

        t.cut_b_a();

        let err = dial_err(&t.a, &t.b.local_addr()).await;
        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_new_dial_from_b_to_a_is_no_route() {
        let t = setup_triangle().await;

        t.cut_b_a();

        let err = dial_err(&t.b, &t.a.local_addr()).await;
        assert!(
            matches!(err, NetError::NoRoute),
            "cut(b, a) = cut(a, b), expected NoRoute: {err:?}"
        );
    }

    #[tokio::test]
    async fn the_live_path_between_a_and_a_third_node_works() {
        let t = setup_triangle().await;

        t.cut_b_a();

        t.a_to_c.send(pkt(9)).await.unwrap();
        assert_eq!(num(&recv_ok(&*t.c_to_a).await), 9);
    }

    #[tokio::test]
    async fn a_new_dial_from_b_to_a_third_node_still_works() {
        let t = setup_triangle().await;

        t.cut_b_a();

        let dialed = dial_ok(&t.b, &t.c.local_addr()).await;
        let arrived = listen_path(&t.c).await;
        dialed.send(pkt(3)).await.unwrap();
        assert_eq!(num(&recv_ok(&*arrived).await), 3);
    }

    #[tokio::test]
    async fn a_restart_of_a_node_does_not_lift_it() {
        let t = setup_triangle().await;
        t.cut_b_a();

        let b_again = t.net.connector(&secret(2));

        let err = dial_err(&t.a, &b_again.local_addr()).await;
        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }
}

mod heal {
    use super::*;

    #[tokio::test]
    async fn lets_a_new_dial_through() {
        let t = setup_triangle().await;
        t.cut_b_a();

        t.net.heal(peer(1), peer(2));

        let dialed = dial_ok(&t.a, &t.b.local_addr()).await;
        let arrived = listen_path(&t.b).await;
        dialed.send(pkt(1)).await.unwrap();
        assert_eq!(num(&recv_ok(&*arrived).await), 1);
    }

    #[tokio::test]
    async fn lifts_the_cut_given_in_the_reverse_order() {
        let t = setup_triangle().await;
        t.net.cut(peer(1), peer(2));

        t.net.heal(peer(2), peer(1));

        dial_ok(&t.a, &t.b.local_addr()).await;
    }

    #[tokio::test]
    async fn does_not_revive_the_old_path() {
        let t = setup_triangle().await;
        t.cut_b_a();

        t.net.heal(peer(1), peer(2));

        assert_eq!(recv_err(&*t.a_to_b).await, ErrorKind::UnexpectedEof);
    }
}

// ---- helpers ----

/// Three nodes: lines A↔B and A↔C (`x_to_y` is the end of X of the line with Y).
struct Triangle {
    net: Arc<MemNet>,
    a: Arc<MemConnector>,
    b: Arc<MemConnector>,
    c: Arc<MemConnector>,
    a_to_b: Box<dyn PacketPath>,
    b_to_a: Box<dyn PacketPath>,
    a_to_c: Box<dyn PacketPath>,
    c_to_a: Box<dyn PacketPath>,
}

impl Triangle {
    /// `cut(b, a)` — in the opposite order to how A dialed B.
    fn cut_b_a(&self) {
        self.net.cut(peer(2), peer(1));
    }
}

async fn setup_triangle() -> Triangle {
    let net = MemNet::new();
    let a = net.connector(&secret(1));
    let b = net.connector(&secret(2));
    let c = net.connector(&secret(3));
    let a_to_b = dial_ok(&a, &b.local_addr()).await;
    let b_to_a = listen_path(&b).await;
    let a_to_c = dial_ok(&a, &c.local_addr()).await;
    let c_to_a = listen_path(&c).await;
    Triangle {
        net,
        a,
        b,
        c,
        a_to_b,
        b_to_a,
        a_to_c,
        c_to_a,
    }
}
