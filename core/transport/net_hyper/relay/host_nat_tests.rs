//! The relay host behind a NAT: an adaptive node tries a relay, a consistent NAT announces the bare key, an open
//! relay announces its address, and a relay that cannot be reached from outside is closed. The step table is
//! checked directly; the spawned host runs on a stand-in relay whose NAT kind the test changes.

use super::*;
use crate::transport::net_hyper::relay::served::{offer_of, Offer, ServedRelay};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use tokio::time::{sleep, timeout};
use townjump_dht_peeroxide::NatKind;

const SECOND: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);
const WAIT: Duration = Duration::from_secs(5);
const KEY: u8 = 7;

mod relay_wanted {
    use super::*;

    #[test]
    fn adaptive_node_behind_nat_serves() {
        // given: an adaptive node that is firewalled and not persistent
        let wanted = relay_wanted(&behind_nat());
        assert!(wanted, "an adaptive node behind a NAT did not try a relay");
    }
}

mod step {
    use super::*;

    mod running_and_wanted {
        use super::*;

        #[test]
        fn consistent_offer_is_advertised() {
            // given: a relay that has just started
            let (state, now) = running();
            let step = state.step(true, Offer::Serve(key_only()), now);
            assert_eq!(step, HostStep::Advertise(key_only()));
        }

        #[test]
        fn unchanged_key_only_advert_stays() {
            // given
            let (state, now) = advertising(key_only());
            let step = state.step(true, Offer::Serve(key_only()), now);
            assert_eq!(step, HostStep::Stay);
        }

        #[test]
        fn address_after_a_key_only_advert_is_advertised() {
            // given: the bare key is announced
            let (state, now) = advertising(key_only());
            // when: the relay turns out to be open
            let step = state.step(true, Offer::Serve(with_address()), now);
            assert_eq!(step, HostStep::Advertise(with_address()));
        }

        #[test]
        fn unfit_offer_restarts_before_any_advert() {
            // given: a relay inside its grace that has announced nothing
            let (state, now) = running();
            let step = state.step(true, Offer::Unfit, now);
            assert_eq!(step, HostStep::Restart, "the relay waited out the grace");
        }

        #[test]
        fn unfit_offer_restarts_an_advertised_relay() {
            // given
            let (state, now) = advertising(key_only());
            let step = state.step(true, Offer::Unfit, now);
            assert_eq!(step, HostStep::Restart, "an unfit relay stayed announced");
        }
    }

    mod running_and_unwanted {
        use super::*;

        #[test]
        fn unfit_offer_restarts_too() {
            // given: the node does not want a relay and nothing is announced
            let (state, now) = running();
            let step = state.step(false, Offer::Unfit, now);
            assert_eq!(step, HostStep::Restart);
        }
    }
}

mod spawned {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn consistent_relay_is_advertised_without_an_address() {
        // given: a relay behind a consistent NAT
        let rig = setup(NatKind::Consistent, None);
        // when
        let advert = rig.next_advert().await;
        // then
        assert_eq!(advert, key_only());
        assert_eq!(rig.starts(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn open_relay_is_advertised_with_its_address() {
        // given: an open relay that knows its address
        let rig = setup(NatKind::Open, Some(address()));
        // when
        let advert = rig.next_advert().await;
        // then
        assert_eq!(advert, with_address());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn random_relay_is_closed_and_never_advertised() {
        // given: a relay behind a random NAT
        let rig = setup(NatKind::Random, None);
        let announced = rig.watch_announcements();
        // when: it starts, is found unfit and the host goes on for a few polls
        rig.wait_until_closed().await;
        sleep(POLL * 4).await;
        // then
        assert!(
            !announced.load(Ordering::SeqCst),
            "an unfit relay was announced"
        );
        assert_eq!(*rig.host.keys().borrow(), None);
        assert_eq!(rig.starts(), 1, "a closed relay was started again at once");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_turning_random_retracts_and_closes() {
        // given: an announced relay behind a consistent NAT
        let rig = setup(NatKind::Consistent, None);
        rig.next_advert().await;
        // when: the NAT starts to map every peer to another port
        rig.set_nat(NatKind::Random, None);
        // then
        rig.keys_where(Option::is_none).await;
        rig.wait_until_closed().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_turning_open_is_readvertised_with_the_address() {
        // given: an announced relay behind a consistent NAT
        let rig = setup(NatKind::Consistent, None);
        rig.next_advert().await;
        // when: the node moves to a public address
        rig.set_nat(NatKind::Open, Some(address()));
        // then: the same relay announces the address
        let advert = rig
            .keys_where(|a| a.is_some_and(|a| a.address.is_some()))
            .await;
        assert_eq!(advert, Some(with_address()));
        assert_eq!(rig.starts(), 1, "the relay was restarted");
    }
}

// --- setup and helpers ---

/// A relay that the test steers: its NAT kind and address live in a cell shared by all its clones.
#[derive(Clone)]
struct FakeRelay {
    nat: Arc<Mutex<(NatKind, Option<SocketAddrV4>)>>,
    closed: Arc<AtomicBool>,
}

impl ServedRelay for FakeRelay {
    fn key(&self) -> [u8; 32] {
        [KEY; 32]
    }

    async fn offer(&self) -> Offer {
        let (kind, address) = *self.nat.lock().unwrap();
        offer_of(self.key(), kind, address)
    }

    async fn close(self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

struct Rig {
    host: RelayHost,
    relay: FakeRelay,
    starts: Arc<AtomicUsize>,
}

fn setup(kind: NatKind, address: Option<SocketAddrV4>) -> Rig {
    let relay = FakeRelay {
        nat: Arc::new(Mutex::new((kind, address))),
        closed: Arc::new(AtomicBool::new(false)),
    };
    let starts = Arc::new(AtomicUsize::new(0));
    let host = spawn_host(Arc::new(behind_nat), start_with(&relay, &starts), POLL);
    Rig {
        host,
        relay,
        starts,
    }
}

fn start_with(relay: &FakeRelay, starts: &Arc<AtomicUsize>) -> StartRelay<FakeRelay> {
    let (relay, starts) = (relay.clone(), starts.clone());
    Arc::new(move || {
        starts.fetch_add(1, Ordering::SeqCst);
        let relay = relay.clone();
        Box::pin(async move { Ok(relay) })
    })
}

impl Rig {
    fn starts(&self) -> usize {
        self.starts.load(Ordering::SeqCst)
    }

    fn set_nat(&self, kind: NatKind, address: Option<SocketAddrV4>) {
        *self.relay.nat.lock().unwrap() = (kind, address);
    }

    /// Waits until the announced advert satisfies `want` and returns it.
    async fn keys_where(&self, want: impl Fn(&Option<RelayAdvert>) -> bool) -> Option<RelayAdvert> {
        let mut adverts = self.host.keys();
        let seen = timeout(WAIT, adverts.wait_for(|a| want(a))).await;
        let seen = seen.expect("the expected advert state never came");
        let advert = seen.expect("the host is alive");
        *advert
    }

    async fn next_advert(&self) -> RelayAdvert {
        self.keys_where(Option::is_some).await.expect("an advert")
    }

    async fn wait_until_closed(&self) {
        let closed = timeout(WAIT, async {
            while !self.relay.closed.load(Ordering::SeqCst) {
                sleep(POLL).await;
            }
        })
        .await;
        assert!(closed.is_ok(), "the relay was not closed");
    }

    /// A flag that turns on when any advert shows up on the host's channel.
    fn watch_announcements(&self) -> Arc<AtomicBool> {
        let announced = Arc::new(AtomicBool::new(false));
        let (flag, mut adverts) = (announced.clone(), self.host.keys());
        task_label::spawn(async move {
            while adverts.changed().await.is_ok() {
                if adverts.borrow().is_some() {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        });
        announced
    }
}

fn behind_nat() -> NodeMode {
    NodeMode {
        adaptive: true,
        persistent: false,
        firewalled: true,
        address: None,
    }
}

fn address() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 9), 40_000)
}

fn key_only() -> RelayAdvert {
    RelayAdvert {
        key: [KEY; 32],
        address: None,
    }
}

fn with_address() -> RelayAdvert {
    RelayAdvert {
        address: Some(address()),
        ..key_only()
    }
}

/// A relay that started a second ago and is waiting for its offer, and the current moment.
fn running() -> (HostState, Instant) {
    let t0 = Instant::now();
    let mut state = HostState::default();
    state.started(t0);
    (state, t0 + SECOND)
}

/// A running relay whose `advert` was announced a second ago, and the current moment.
fn advertising(advert: RelayAdvert) -> (HostState, Instant) {
    let (mut state, now) = running();
    state.advertised(Some(advert), now - SECOND);
    (state, now)
}
