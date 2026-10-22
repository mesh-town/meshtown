//! Peer admission on the server: open or by a "peer, service" predicate.

use super::*;
use crate::net::id::NodeSecret;
use crate::net::service::{AUTH, EXIT};

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn admit(f: impl Fn(PeerId, &ServiceId) -> bool + Send + Sync + 'static) -> Option<Admit> {
    Some(Arc::new(f))
}

#[test]
fn open_gate_admits_all() {
    let gate = PeerGate::open();
    assert!(gate.admits(id(1), &AUTH));
    assert!(gate.admits(id(2), &EXIT));
}

#[test]
fn admit_predicate_decides() {
    let gate = PeerGate::open();
    gate.set_admit(admit(|peer, svc| peer == id(1) || *svc == AUTH));
    assert!(gate.admits(id(1), &EXIT), "a member is admitted to any service");
    assert!(gate.admits(id(3), &AUTH), "an outsider is admitted to AUTH");
    assert!(!gate.admits(id(3), &EXIT), "an outsider is not admitted to EXIT");
}

#[test]
fn panicking_predicate_refuses() {
    let gate = PeerGate::open();
    gate.set_admit(admit(|_, _| panic!("predicate failed")));
    assert!(!gate.admits(id(1), &AUTH));
    gate.set_admit(None);
    assert!(gate.admits(id(1), &AUTH), "the lock is not poisoned");
}

#[test]
fn admit_can_be_cleared() {
    let gate = PeerGate::open();
    gate.set_admit(admit(|_, _| false));
    assert!(!gate.admits(id(1), &AUTH));
    gate.set_admit(None);
    assert!(gate.admits(id(1), &EXIT));
}
