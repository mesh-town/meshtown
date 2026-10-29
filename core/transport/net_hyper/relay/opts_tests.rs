//! Options of the relay node: the admission check on the key of a peer and the fixed settings.

use super::*;
use crate::transport::net::contract::secret;
use crate::transport::net_hyper::keys::key_pair;

mod firewall {
    use super::*;

    #[test]
    fn admitted_peer_passes() {
        // given
        let firewall = firewall_admitting(&[peer(1)]);
        // when
        let refused = firewall(peer(1).as_bytes());
        // then
        assert!(!refused, "an admitted peer was refused");
    }

    #[test]
    fn stranger_is_refused() {
        // given
        let firewall = firewall_admitting(&[peer(1)]);
        // when
        let refused = firewall(peer(4).as_bytes());
        // then
        assert!(refused, "a peer that is not admitted passed");
    }

    #[test]
    fn unparsable_key_is_refused() {
        // given: admit would let anyone in, but the key is not a node identity
        let firewall = firewall_of(Arc::new(|_| true));
        let mut off = [0u8; 32];
        off[0] = 2; // y = 2: there is no such point on the ed25519 curve
                    // when
        let refused = firewall(&off);
        // then
        assert!(refused, "a key off the curve passed");
    }

    #[test]
    fn admit_is_asked_about_the_peer_of_the_key() {
        // given: admit records whom it was asked about
        let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = asked.clone();
        let firewall = firewall_of(Arc::new(move |p| {
            seen.lock().unwrap().push(p);
            true
        }));
        // when
        firewall(peer(2).as_bytes());
        // then
        assert_eq!(*asked.lock().unwrap(), vec![peer(2)]);
    }

    #[test]
    fn verdict_follows_the_admission_at_every_call() {
        // given: admit reads a flag that flips between two calls
        let open = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = open.clone();
        let firewall = firewall_of(Arc::new(move |_| {
            flag.load(std::sync::atomic::Ordering::SeqCst)
        }));
        // when
        let before = firewall(peer(1).as_bytes());
        open.store(false, std::sync::atomic::Ordering::SeqCst);
        let after = firewall(peer(1).as_bytes());
        // then
        assert!(!before, "the peer was refused while admitted");
        assert!(after, "the peer passed after the admission was withdrawn");
    }
}

mod options {
    use super::*;

    #[test]
    fn identity_is_the_given_pair() {
        // given
        let pair = key_pair(&secret(9));
        // when
        let opts = relay_opts(pair.clone(), Arc::new(|_| true));
        // then
        assert_eq!(opts.identity, Some(pair));
    }

    #[test]
    fn announce_follows_the_constant() {
        // given / when
        let opts = relay_opts(key_pair(&secret(9)), Arc::new(|_| true));
        // then
        assert_eq!(opts.announce, RELAY_ANNOUNCE);
    }

    #[test]
    fn port_is_any() {
        // given / when
        let opts = relay_opts(key_pair(&secret(9)), Arc::new(|_| true));
        // then
        assert_eq!(opts.port, 0);
    }

    #[test]
    fn firewall_is_set() {
        // given / when
        let opts = relay_opts(key_pair(&secret(9)), Arc::new(|_| true));
        // then
        assert!(opts.firewall.is_some(), "the relay would admit everyone");
    }
}

// --- helpers ---

fn peer(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

/// The firewall of a relay that admits exactly `admitted`.
fn firewall_admitting(admitted: &[PeerId]) -> Firewall {
    let admitted = admitted.to_vec();
    firewall_of(Arc::new(move |p| admitted.contains(&p)))
}

/// The firewall that `relay_opts` builds around `admit`.
fn firewall_of(admit: RelayAdmit) -> Firewall {
    relay_opts(key_pair(&secret(9)), admit)
        .firewall
        .expect("the relay has a firewall")
}
