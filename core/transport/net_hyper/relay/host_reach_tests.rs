//! The step table of the relay host: start, announce, move, retract and restart follow the node mode, the
//! address the relay has confirmed and the clock.

use super::*;
use crate::transport::net_hyper::test_cluster::cluster;
use crate::transport::net_hyper::test_relay::relay_on;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Mutex;
use tokio::time::timeout;

const SECOND: Duration = Duration::from_secs(1);

mod not_running {
    use super::*;

    #[test]
    fn fresh_wanted_node_starts() {
        // given
        let state = HostState::default();
        // when
        let step = state.step(true, Offer::Wait, Instant::now());
        // then
        assert_eq!(step, HostStep::Start);
    }

    #[test]
    fn fresh_unwanted_node_stays() {
        assert_eq!(
            HostState::default().step(false, Offer::Wait, Instant::now()),
            HostStep::Stay
        );
    }

    #[test]
    fn restart_waits_for_the_retry_window() {
        // given: a relay that found no address was closed
        let closed_at = Instant::now();
        let state = stopped_at(closed_at);
        // when
        let early = state.step(true, Offer::Wait, closed_at + RELAY_RETRY - SECOND);
        let due = state.step(true, Offer::Wait, closed_at + RELAY_RETRY);
        // then
        assert_eq!(early, HostStep::Stay, "a relay started inside the window");
        assert_eq!(due, HostStep::Start, "no relay after the window");
    }

    #[test]
    fn unwanted_node_waits_out_the_window_too() {
        // given
        let closed_at = Instant::now();
        let state = stopped_at(closed_at);
        // when
        let step = state.step(false, Offer::Wait, closed_at + RELAY_RETRY);
        // then
        assert_eq!(step, HostStep::Stay);
    }
}

mod running_and_wanted {
    use super::*;

    #[test]
    fn seen_address_is_advertised() {
        // given: a relay that has just confirmed its address
        let t0 = Instant::now();
        let state = running_at(t0);
        // when
        let step = state.step(true, Offer::Serve(advert(1)), t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Advertise(advert(1)));
    }

    #[test]
    fn changed_address_is_advertised_again() {
        // given: the first address is announced
        let t0 = Instant::now();
        let state = advertising_at(advert(1), t0);
        // when: the relay reports another one
        let step = state.step(true, Offer::Serve(advert(2)), t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Advertise(advert(2)));
    }

    #[test]
    fn unchanged_address_stays() {
        // given
        let t0 = Instant::now();
        let state = advertising_at(advert(1), t0);
        // when
        let step = state.step(true, Offer::Serve(advert(1)), t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Stay);
    }

    #[test]
    fn lost_address_retracts() {
        // given: an announced address
        let t0 = Instant::now();
        let state = advertising_at(advert(1), t0);
        // when: the relay no longer knows it
        let step = state.step(true, Offer::Wait, t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Retract);
    }

    #[test]
    fn address_back_after_a_retraction_is_advertised_without_a_new_start() {
        // given: the announcement was retracted, the relay is still up
        let t0 = Instant::now();
        let state = retracted_at(advert(1), t0);
        // when
        let step = state.step(true, Offer::Serve(advert(1)), t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Advertise(advert(1)));
    }

    #[test]
    fn no_address_past_grace_restarts() {
        // given: a relay that never confirmed an address
        let t0 = Instant::now();
        let state = running_at(t0);
        // when
        let step = state.step(true, Offer::Wait, t0 + REACH_GRACE);
        // then
        assert_eq!(step, HostStep::Restart);
    }

    #[test]
    fn no_address_within_grace_stays() {
        // given
        let t0 = Instant::now();
        let state = running_at(t0);
        // when
        let step = state.step(true, Offer::Wait, t0 + REACH_GRACE - SECOND);
        // then
        assert_eq!(step, HostStep::Stay);
    }

    #[test]
    fn grace_counts_from_the_retraction() {
        // given: a relay up for long, whose address was lost and retracted just now
        let started = Instant::now();
        let retracted = started + REACH_GRACE * 2;
        let state = retracted_at(advert(1), retracted);
        // when: a full grace has passed since the start but not since the retraction
        let step = state.step(true, Offer::Wait, retracted + REACH_GRACE - SECOND);
        // then
        assert_eq!(step, HostStep::Stay, "the grace was counted from the start");
    }
}

mod running_and_unwanted {
    use super::*;

    #[test]
    fn unwanted_node_retracts() {
        // given: an announced address
        let t0 = Instant::now();
        let state = advertising_at(advert(1), t0);
        // when: the node is not public any more, the relay still reports the address
        let step = state.step(false, Offer::Serve(advert(1)), t0 + SECOND);
        // then
        assert_eq!(step, HostStep::Retract);
    }

    #[test]
    fn unwanted_node_without_an_announcement_stays() {
        // given: a relay that announced nothing, past its grace
        let t0 = Instant::now();
        let state = running_at(t0);
        // when
        let step = state.step(false, Offer::Wait, t0 + REACH_GRACE * 2);
        // then: the mode decides, not the wait for an address
        assert_eq!(step, HostStep::Stay);
    }
}

mod spawned_host {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn spawned_host_reports_the_address_of_its_relay() {
        // given: a public node whose relay is assumed open on the loopback
        let cl = cluster(3).await;
        let relay = relay_on(&cl).await;
        let want = RelayAdvert {
            key: relay.public_key(),
            address: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, relay.local_port())),
        };
        let host = spawn_host(
            Arc::new(public_mode),
            start_with(relay),
            Duration::from_millis(50),
        );
        // when
        let mut adverts = host.keys();
        let seen = timeout(Duration::from_secs(60), adverts.wait_for(|a| a.is_some())).await;
        let advert = *seen
            .expect("an advert was announced")
            .expect("the host is alive");
        // then: the key of the relay and the port it listens on
        assert_eq!(advert, Some(want));
        host.close().await;
    }
}

// --- setup and helpers ---

/// A distinct advert per `n`: key `[n; 32]`, address `10.0.0.n:4000+n`.
fn advert(n: u8) -> RelayAdvert {
    RelayAdvert {
        key: [n; 32],
        address: Some(SocketAddrV4::new(
            Ipv4Addr::new(10, 0, 0, n),
            4000 + u16::from(n),
        )),
    }
}

/// A relay started at `at`, waiting for its address.
fn running_at(at: Instant) -> HostState {
    let mut state = HostState::default();
    state.started(at);
    state
}

/// A running relay whose `advert` was announced at `at`.
fn advertising_at(advert: RelayAdvert, at: Instant) -> HostState {
    let mut state = running_at(at);
    state.advertised(Some(advert), at);
    state
}

/// A running relay whose announcement was retracted at `at` (it had announced `advert`).
fn retracted_at(advert: RelayAdvert, at: Instant) -> HostState {
    let mut state = advertising_at(advert, at);
    state.advertised(None, at);
    state
}

/// A host whose relay was closed at `at`.
fn stopped_at(at: Instant) -> HostState {
    let mut state = running_at(at);
    state.stopped(at);
    state
}

fn public_mode() -> NodeMode {
    NodeMode {
        persistent: true,
        adaptive: false,
        firewalled: false,
        address: None,
    }
}

/// A start that hands out `relay` the first time and fails afterwards.
fn start_with(relay: PeeroxideRelay) -> StartRelay {
    let slot = Arc::new(Mutex::new(Some(relay)));
    Arc::new(move || {
        let taken = slot.lock().unwrap().take();
        Box::pin(
            async move { taken.ok_or_else(|| DhtError::Backend("relay already taken".into())) },
        )
    })
}
