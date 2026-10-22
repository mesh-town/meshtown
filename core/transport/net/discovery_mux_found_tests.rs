//! Tests for discovery merging, continued: the `found` tap (every source message) and `state` cleanup.

use super::*;
use crate::transport::net::discovery_mux::lock;
use tokio::sync::mpsc;

/// A mux with a `tx` tap and `scripts` sources; before `start` the caller can subscribe to news.
fn tapped_mux(scripts: &[&Arc<Script>], tx: mpsc::Sender<PeerAddr>, ttl_ms: u64) -> DiscoveryMux {
    let mut mux = setup_with(&["mainline"], ttl_ms);
    for script in scripts {
        let source: Arc<dyn Discovery> = (*script).clone();
        mux.add(source);
    }
    mux.set_found_tap(tx);
    mux
}

fn many(count: u8) -> Vec<PeerAddr> {
    (1..=count).map(|n| addr(n, &[1])).collect()
}

fn state_ids(mux: &DiscoveryMux) -> Vec<PeerId> {
    lock(&mux.state).keys().copied().collect()
}

mod tap {
    use super::*;

    #[tokio::test]
    async fn tap_gets_every_report() {
        let a = addr(2, &[1]);
        let script = Script::new("mainline")
            .finding(vec![a.clone(), a.clone()])
            .shared();
        let (tx, mut rx) = mpsc::channel(8);
        let mux = Arc::new(tapped_mux(&[&script], tx, TTL_MS));
        let mut news = mux.subscribe();

        mux.start();

        assert_eq!(within("first", rx.recv()).await, Some(a.clone()));
        assert_eq!(within("second", rx.recv()).await, Some(a.clone()));
        assert_eq!(within("news", news.recv()).await.ok(), Some(a));
        sleep(Duration::from_millis(50)).await;
        assert!(matches!(news.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn tap_gets_report_without_hints() {
        let bare = addr(2, &[]);
        let script = Script::new("mainline").finding(vec![bare.clone()]).shared();
        let (tx, mut rx) = mpsc::channel(8);
        let mux = Arc::new(tapped_mux(&[&script], tx, TTL_MS));

        mux.start();

        assert_eq!(within("tap", rx.recv()).await, Some(bare));
        assert_eq!(mux.lookup(&pid(2)), None);
    }

    #[tokio::test]
    async fn closed_tap_stops_source() {
        let script = Script::new("mainline").finding(many(70)).shared();
        let (tx, rx) = mpsc::channel(8);
        drop(rx);
        let mux = Arc::new(tapped_mux(&[&script], tx, TTL_MS));

        mux.start();

        wait_until("source stopped", || script.dropped()).await;
        drop(mux);
    }

    #[tokio::test]
    async fn slow_tap_keeps_order_without_loss() {
        let script = Script::new("mainline").finding(many(10)).shared();
        let (tx, mut rx) = mpsc::channel(1);
        let mux = Arc::new(tapped_mux(&[&script], tx, TTL_MS));

        mux.start();

        let mut got = Vec::new();
        for _ in 0..10 {
            sleep(Duration::from_millis(20)).await;
            got.push(within("address", rx.recv()).await.map(|a| a.id));
        }
        let want: Vec<_> = (1..=10).map(|n| Some(pid(n))).collect();
        assert_eq!(got, want);
    }

    #[tokio::test]
    async fn pump_releases_mux_while_waiting() {
        let script = Script::new("mainline").finding(many(5)).shared();
        let (tx, _rx) = mpsc::channel(1);
        let mux = Arc::new(tapped_mux(&[&script], tx, TTL_MS));
        mux.start();
        sleep(Duration::from_millis(150)).await;

        drop(mux);

        wait_until("source stopped", || script.dropped()).await;
    }

    #[tokio::test]
    async fn no_tap_old_behaviour() {
        let script = Script::new("mainline").finding(many(70)).shared();
        let mux = started(&["mainline"], &[&script]);

        let last = within("last", wait_lookup(&mux, 70)).await;

        assert_eq!(last.id, pid(70));
        assert!(mux.lookup(&pid(1)).is_some());
    }
}

mod sweep {
    use super::*;

    fn plain(ttl_ms: u64) -> DiscoveryMux {
        setup_with(&["mainline"], ttl_ms)
    }

    #[tokio::test]
    async fn sweep_drops_dead_keeps_live() {
        let mux = plain(50);
        mux.note("mainline", addr(1, &[1]));
        sleep(Duration::from_millis(80)).await;
        mux.note("mainline", addr(2, &[2]));

        mux.sweep();

        assert_eq!(state_ids(&mux), vec![pid(2)]);
    }

    #[tokio::test]
    async fn sweeper_runs_after_start() {
        let mux = Arc::new(plain(50));
        mux.start();
        mux.note("mainline", addr(1, &[1]));

        sleep(Duration::from_millis(150)).await;

        assert!(
            state_ids(&mux).is_empty(),
            "state is cleaned without lookup"
        );
    }

    #[tokio::test]
    async fn stop_ends_sweeper() {
        let mux = Arc::new(plain(50));
        mux.start();
        mux.stop().await;
        mux.note("mainline", addr(1, &[1]));

        sleep(Duration::from_millis(150)).await;

        assert_eq!(state_ids(&mux), vec![pid(1)]);
    }
}
