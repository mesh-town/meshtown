//! The relay host: when a node serves a relay and the task that follows the node mode (the step table is in
//! `host_reach_tests.rs`).

use super::*;
use crate::transport::net_hyper::test_cluster::{cluster, Cluster};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use townjump_dht_peeroxide::PeeroxideFactory;

const POLL: Duration = Duration::from_millis(50);
const WAIT: Duration = Duration::from_secs(60);

mod relay_wanted {
    use super::*;

    #[test]
    fn public_persistent_node_serves() {
        assert!(relay_wanted(&mode(true, false)));
    }

    #[test]
    fn firewalled_node_does_not() {
        assert!(!relay_wanted(&mode(true, true)));
    }

    #[test]
    fn ephemeral_node_does_not() {
        assert!(!relay_wanted(&mode(false, false)));
    }
}

mod spawned_host {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn spawn_starts_and_reports_its_advert() {
        // given: a public node
        let mut rig = rig(true, 0).await;
        // when: the host runs
        let advert = rig.next_advert().await;
        // then: the announced advert is the one of the single relay that was started
        assert_eq!(rig.started(), vec![advert]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn firewalled_node_starts_no_relay() {
        // given: a node behind a firewall
        let rig = rig(false, 0).await;
        // when: several polls pass
        tokio::time::sleep(POLL * 6).await;
        // then: no relay was started and nothing is announced
        assert_eq!(rig.attempts.load(Ordering::SeqCst), 0);
        assert_eq!(*rig.host.keys().borrow(), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn failed_start_is_retried_next_poll() {
        // given: a public node whose first relay start fails
        let mut rig = rig(true, 1).await;
        // when: the host runs
        let advert = rig.next_advert().await;
        // then: the second attempt succeeded and announced its relay
        assert_eq!(rig.attempts.load(Ordering::SeqCst), 2);
        assert_eq!(rig.started(), vec![advert]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn losing_and_regaining_a_public_address_keeps_one_relay() {
        // given: a running host that announced its advert
        let mut rig = rig(true, 0).await;
        let advert = rig.next_advert().await;
        // when: the node becomes firewalled, then public again
        rig.public.store(false, Ordering::SeqCst);
        rig.wait_for_none().await;
        rig.public.store(true, Ordering::SeqCst);
        // then: the same advert is announced again and the relay was started once
        assert_eq!(rig.next_advert().await, advert);
        assert_eq!(rig.started(), vec![advert]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn close_reports_none_and_closes_the_relay() {
        // given: a running host that announced its advert
        let mut rig = rig(true, 0).await;
        rig.next_advert().await;
        let mut keys = rig.host.keys();
        // when: the host is closed
        rig.host.close().await;
        // then: the channel reads None, and its sender is gone: the task has ended
        assert_eq!(*keys.borrow(), None);
        let ended =
            tokio::time::timeout(WAIT, async { while keys.changed().await.is_ok() {} }).await;
        assert!(ended.is_ok(), "the host task is still alive after close");
    }
}

mod hanging_start {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn close_during_a_hanging_start_returns() {
        // given: a public node whose relay start never completes
        let start: StartRelay = Arc::new(|| Box::pin(std::future::pending()));
        let host = spawn_host(Arc::new(|| mode(true, false)), start, POLL);
        tokio::task::yield_now().await;
        // when: the host is closed
        let began = tokio::time::Instant::now();
        host.close().await;
        // then: close returns at once, without waiting for the task to give up
        let took = began.elapsed();
        assert!(
            took < HOST_CLOSE_WAIT,
            "close waited {took:?} for a hanging start"
        );
    }
}

/// A host over a loopback cluster, with the facts it started.
struct Rig {
    host: RelayHost,
    public: Arc<AtomicBool>,
    attempts: Arc<AtomicUsize>,
    started: Arc<Mutex<Vec<RelayAdvert>>>,
    _cluster: Cluster,
}

impl Rig {
    /// Waits until an advert is announced and returns it.
    async fn next_advert(&mut self) -> RelayAdvert {
        let mut adverts = self.host.keys();
        let seen = tokio::time::timeout(WAIT, adverts.wait_for(|a| a.is_some())).await;
        let advert = *seen
            .expect("an advert was announced")
            .expect("the host is alive");
        advert.expect("an advert")
    }

    async fn wait_for_none(&mut self) {
        let mut adverts = self.host.keys();
        let seen = tokio::time::timeout(WAIT, adverts.wait_for(|a| a.is_none())).await;
        seen.expect("the advert was retracted")
            .expect("the host is alive");
    }

    /// Adverts (key and loopback address) of the relays that started.
    fn started(&self) -> Vec<RelayAdvert> {
        self.started.lock().unwrap().clone()
    }
}

/// A host whose node is `public` (can change later) and whose first `fail_first` starts fail.
async fn rig(public: bool, fail_first: usize) -> Rig {
    let cl = cluster(3).await;
    let public = Arc::new(AtomicBool::new(public));
    let attempts = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Mutex::new(Vec::new()));
    let start = start_fn(
        cl.bootstrap(),
        fail_first,
        attempts.clone(),
        started.clone(),
    );
    let flag = public.clone();
    let mode_now = Arc::new(move || mode(true, !flag.load(Ordering::SeqCst)));
    let host = spawn_host(mode_now, start, POLL);
    Rig {
        host,
        public,
        attempts,
        started,
        _cluster: cl,
    }
}

fn start_fn(
    bootstrap: Vec<String>,
    fail_first: usize,
    attempts: Arc<AtomicUsize>,
    started: Arc<Mutex<Vec<RelayAdvert>>>,
) -> StartRelay {
    Arc::new(move || {
        let n = attempts.fetch_add(1, Ordering::SeqCst);
        let (bootstrap, started) = (bootstrap.clone(), started.clone());
        Box::pin(async move {
            if n < fail_first {
                return Err(DhtError::Backend("no route to the DHT".into()));
            }
            let factory = PeeroxideFactory {
                bootstrap: Some(bootstrap),
                ..Default::default()
            };
            let relay = factory.create_relay(None).await?;
            let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, relay.local_port());
            let advert = RelayAdvert {
                key: relay.public_key(),
                address: Some(address),
            };
            started.lock().unwrap().push(advert);
            Ok(relay)
        })
    })
}

fn mode(persistent: bool, firewalled: bool) -> NodeMode {
    NodeMode {
        persistent,
        adaptive: false,
        firewalled,
        address: None,
    }
}
