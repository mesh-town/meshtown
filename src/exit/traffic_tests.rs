//! Tests of the exit speed registry: the `add` + `rates_at` window on explicit `Instant`s, the `watch` task in
//! real time (the project's `tokio` has no `test-util`).

use super::*;
use crate::net::facade::test_support::{pid, secret, within, Probe, Spy};
use crate::net::mem::MemNet;
use crate::net::service::EXIT;
use crate::net::{NetStack, NetStackBuilder};
use tokio::io::AsyncWriteExt;

mod window {
    use super::*;

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn rate_is_window_sum_over_ten_seconds() {
        let (traffic, t0) = (ExitTraffic::default(), Instant::now());
        traffic.add(pid(2), Direction::ThroughUs, 100_000, t0);

        let rates = traffic.rates_at(t0 + SEC);

        assert_eq!(rates[&pid(2)].through_us_bps, 10_000);
    }

    #[test]
    fn samples_older_than_window_are_dropped() {
        let (traffic, t0) = (ExitTraffic::default(), Instant::now());
        traffic.add(pid(2), Direction::ThroughUs, 100_000, t0);

        let rates = traffic.rates_at(t0 + 11 * SEC);

        assert!(!rates.contains_key(&pid(2)));
    }

    #[test]
    fn directions_are_kept_apart() {
        let (traffic, t0) = (ExitTraffic::default(), Instant::now());
        traffic.add(pid(2), Direction::ThroughPeer, 50_000, t0);
        traffic.add(pid(2), Direction::ThroughUs, 20_000, t0);

        let rates = traffic.rates_at(t0 + SEC);

        let want = PeerRate { through_peer_bps: 5_000, through_us_bps: 2_000 };
        assert_eq!(rates[&pid(2)], want);
    }

    #[test]
    fn peers_are_kept_apart() {
        let (traffic, t0) = (ExitTraffic::default(), Instant::now());
        traffic.add(pid(2), Direction::ThroughUs, 10_000, t0);
        traffic.add(pid(3), Direction::ThroughPeer, 30_000, t0);

        let rates = traffic.rates_at(t0 + SEC);

        assert_eq!(rates.len(), 2);
        assert_eq!(rates[&pid(2)].through_us_bps, 1_000);
        assert_eq!(rates[&pid(3)].through_peer_bps, 3_000);
    }

    #[test]
    fn zero_bytes_are_not_recorded() {
        let (traffic, t0) = (ExitTraffic::default(), Instant::now());
        traffic.add(pid(2), Direction::ThroughUs, 0, t0);

        assert!(traffic.rates_at(t0 + SEC).is_empty());
    }
}

mod watch {
    use super::*;

    #[tokio::test]
    async fn written_bytes_show_up_within_three_seconds() {
        // given: a mem connection watched by the registry, and a stream with bytes written
        let (_stacks, conn) = connected().await;
        let traffic = ExitTraffic::default();
        traffic.watch(conn.clone(), Direction::ThroughUs);
        let mut pair = within("open", conn.open()).await.expect("stream");
        pair.send.write_all(&[7u8; 10_000]).await.expect("write");

        // when: we wait at most three seconds
        let seen = tokio::time::timeout(Duration::from_secs(3), async {
            while traffic.rates_at(Instant::now()).get(&conn.peer()).is_none_or(|r| r.through_us_bps == 0) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        // then: the peer's speed is nonzero
        assert!(seen.is_ok(), "speed did not appear within 3 s");
    }

    #[tokio::test]
    async fn closed_conn_releases_the_task() {
        // given: a `watch` task on a connection
        let (_stacks, conn) = connected().await;
        let before = Arc::strong_count(&conn);
        let traffic = ExitTraffic::default();
        traffic.watch(conn.clone(), Direction::ThroughUs);

        // when: the connection is closed
        conn.close(0, b"done");
        let released = tokio::time::timeout(Duration::from_secs(3), async {
            while Arc::strong_count(&conn) > before {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        // then: the task released the `Arc`, the sum no longer grows
        assert!(released.is_ok(), "the task did not release the connection within 3 s");
        let first = traffic.rates_at(Instant::now());
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(traffic.rates_at(Instant::now()), first);
    }
}

/// A client (node 1) connection to an exit (node 2) on the mem network; the stacks are returned so they live to the end of the test.
pub(super) async fn connected() -> ((NetStack, NetStack), Arc<dyn PeerConn>) {
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, Probe::new())
        .build()
        .expect("exit built");
    let client = NetStackBuilder::new(secret(1))
        .transport(Spy::on(&net, &secret(1)).shared())
        .build()
        .expect("client built");
    client.add_hints(server.invite_addrs()[0].clone());
    let conn = within("connect", client.connect(pid(2), EXIT))
        .await
        .expect("connection to the exit");
    ((client, server), conn)
}
