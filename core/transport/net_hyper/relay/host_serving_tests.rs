//! The `serving` flag of the relay host: set while a pair of other members goes through the relay, held until
//! the next tick, cleared when the relay stops. Time is virtual, so a tick of the real period costs nothing.

use super::*;
use crate::transport::net_hyper::relay::served::{offer_of, Offer, RelayLoad, ServedRelay};
use std::sync::Mutex;
use tokio::time::timeout;
use townjump_dht_peeroxide::NatKind;

const KEY: u8 = 9;

#[derive(Clone, Default)]
struct LoadedRelay {
    load: Arc<Mutex<RelayLoad>>,
}

impl ServedRelay for LoadedRelay {
    fn key(&self) -> [u8; 32] {
        [KEY; 32]
    }

    async fn offer(&self) -> Offer {
        offer_of(self.key(), NatKind::Consistent, None)
    }

    fn load(&self) -> RelayLoad {
        *self.load.lock().unwrap()
    }

    async fn close(self) {}
}

fn node() -> NodeMode {
    NodeMode {
        adaptive: true,
        persistent: false,
        firewalled: true,
        address: None,
    }
}

/// A host over a relay whose load the test sets; returns once the first tick has announced the relay.
async fn running() -> (RelayHost, LoadedRelay) {
    let relay = LoadedRelay::default();
    let shared = relay.clone();
    let start: StartRelay<LoadedRelay> = Arc::new(move || {
        let relay = shared.clone();
        Box::pin(async move { Ok(relay) })
    });
    let host = spawn_host(Arc::new(node), start, RELAY_MODE_POLL);
    let mut keys = host.keys();
    timeout(RELAY_MODE_POLL * 2, keys.wait_for(|k| k.is_some()))
        .await
        .expect("the relay was announced")
        .expect("the host is alive");
    (host, relay)
}

fn set(relay: &LoadedRelay, sessions_active: i64, pairings_matched: u64) {
    *relay.load.lock().unwrap() = RelayLoad {
        sessions_active,
        pairings_matched,
    };
}

/// Waits for the flag to take `want`; a tick must come for that, so a stale value never passes.
async fn becomes(host: &RelayHost, want: bool) {
    let mut serving = host.serving();
    let seen = timeout(RELAY_MODE_POLL * 2, serving.wait_for(|s| *s == want)).await;
    assert!(seen.is_ok(), "serving did not become {want}");
}

#[tokio::test(start_paused = true)]
async fn serving_follows_sessions() {
    // given: a running relay with no sessions
    let (host, relay) = running().await;
    assert!(!*host.serving().borrow());
    // when: a session opens, then closes without any new pairing
    set(&relay, 1, 0);
    becomes(&host, true).await;
    set(&relay, 0, 0);
    // then: the next tick clears the flag
    becomes(&host, false).await;
}

#[tokio::test(start_paused = true)]
async fn serving_sticks_when_a_pair_came_and_went() {
    // given: a running relay
    let (host, relay) = running().await;
    // when: a pairing completed between two ticks and no session is open any more
    set(&relay, 0, 1);
    // then: this tick reports it, and the following one, with no news, clears it
    becomes(&host, true).await;
    becomes(&host, false).await;
}

#[tokio::test(start_paused = true)]
async fn serving_is_false_after_relay_stops() {
    // given: a relay that carries a pair
    let (host, relay) = running().await;
    set(&relay, 2, 3);
    becomes(&host, true).await;
    // when: the host stops
    let serving = host.serving();
    host.close().await;
    // then: the flag is cleared
    assert!(!*serving.borrow());
}
