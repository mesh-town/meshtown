//! Relay candidates: who may be offered as a relay for a dial, and how the adverts are read from the replica.
//! Only a live, admitted member other than the caller and the target counts; the list is sorted by member.

use super::*;
use crate::members::access::tally::Ledger;
use crate::members::access::view::{AccessView, Verdicts};
use crate::transport::net::contract::secret;
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::ReplicaEntry;
use std::collections::BTreeSet;

const ME: u8 = 1;
const TARGET: u8 = 2;

mod admitted_connected_member {
    use super::*;

    #[test]
    fn with_an_advert_is_a_candidate() {
        // given
        let fx = setup(&[5]);
        // when
        let got = fx.candidates(&[(5, key(7).to_vec())]);
        // then
        assert_eq!(got, vec![candidate(5, 7)]);
    }

    #[test]
    fn without_an_advert_is_absent() {
        // given: members 5 and 6 are live, only 5 advertises
        let fx = setup(&[5, 6]);
        // when
        let got = fx.candidates(&[(5, key(7).to_vec())]);
        // then
        assert_eq!(got, vec![candidate(5, 7)]);
    }

    #[test]
    fn candidates_are_sorted_by_member() {
        // given: the adverts arrive in no particular order
        let fx = setup(&[5, 6, 7, 8]);
        // when
        let got = fx.candidates(&[
            (7, key(1).to_vec()),
            (5, key(2).to_vec()),
            (8, key(3).to_vec()),
            (6, key(4).to_vec()),
        ]);
        // then
        let members: Vec<PeerId> = got.iter().map(|c| c.member).collect();
        let mut sorted = members.clone();
        sorted.sort();
        assert_eq!(members.len(), 4);
        assert_eq!(members, sorted);
    }
}

mod author_is_left_out {
    use super::*;

    #[test]
    fn unadmitted_author_is_skipped() {
        // given: 5 holds a verified connection but the list does not admit it
        let fx = setup_admitting(&[], &[5]);
        // when
        let got = fx.candidates(&[(5, key(7).to_vec())]);
        // then
        assert!(got.is_empty(), "an unadmitted author was offered: {got:?}");
    }

    #[test]
    fn member_without_connection_is_skipped() {
        // given: 5 is admitted but not connected
        let fx = setup_admitting(&[5], &[]);
        // when
        let got = fx.candidates(&[(5, key(7).to_vec())]);
        // then
        assert!(
            got.is_empty(),
            "a member without a connection was offered: {got:?}"
        );
    }

    #[test]
    fn self_is_skipped() {
        // given: even a connection to oneself does not make oneself a candidate
        let fx = setup(&[ME]);
        // when
        let got = fx.candidates(&[(ME, key(7).to_vec())]);
        // then
        assert!(
            got.is_empty(),
            "the caller was offered as its own relay: {got:?}"
        );
    }

    #[test]
    fn target_is_skipped() {
        // given: the target of the dial advertises a relay of its own
        let fx = setup(&[TARGET, 5]);
        // when
        let got = fx.candidates(&[(TARGET, key(7).to_vec()), (5, key(8).to_vec())]);
        // then
        assert_eq!(got, vec![candidate(5, 8)]);
    }
}

mod advert_value {
    use super::*;

    #[test]
    fn short_value_is_skipped() {
        // given
        let fx = setup(&[5]);
        // when
        let got = fx.candidates(&[(5, vec![7; 31])]);
        // then
        assert!(
            got.is_empty(),
            "a 31-byte value was taken for a key: {got:?}"
        );
    }

    #[test]
    fn exact_value_is_the_key() {
        // given
        let fx = setup(&[5]);
        // when
        let got = fx.candidates(&[(5, vec![7; 32])]);
        // then
        assert_eq!(got, vec![candidate(5, 7)]);
    }

    #[test]
    fn long_value_gives_first_32_bytes() {
        // given: the tail past the key is an address and room for a future field
        let fx = setup(&[5]);
        let mut value = vec![7; 32];
        value.extend_from_slice(&[9; 8]);
        // when
        let got = fx.candidates(&[(5, value)]);
        // then: the key is the first 32 bytes whatever follows
        let keys: Vec<[u8; 32]> = got.iter().map(|c| c.relay).collect();
        assert_eq!(keys, vec![key(7)]);
    }
}

mod source {
    use super::*;

    #[tokio::test]
    async fn reads_the_adverts_of_its_own_replica() {
        // given
        let fx = setup(&[5]);
        let replica = replica_of(ME);
        advertise(&replica, 5, 7);
        let source = relay_source(fx.members.clone(), replica, peer(ME));
        // when
        let got = source(peer(TARGET)).await;
        // then
        assert_eq!(got, vec![candidate(5, 7)]);
    }

    #[tokio::test]
    async fn source_reads_only_its_own_replica() {
        // given: the advert of a connected member lies in the replica of another network
        let fx = setup(&[5]);
        let own = replica_of(ME);
        let other = replica_of(ME);
        advertise(&other, 5, 7);
        let source = relay_source(fx.members.clone(), own, peer(ME));
        // when
        let got = source(peer(TARGET)).await;
        // then
        assert!(
            got.is_empty(),
            "an advert of another replica was offered: {got:?}"
        );
    }

    #[tokio::test]
    async fn retracted_advert_is_absent() {
        // given: the member retracted its advert (a tombstone)
        let fx = setup(&[5]);
        let replica = replica_of(ME);
        replica.put_raw(entry(5, b""));
        let source = relay_source(fx.members.clone(), replica, peer(ME));
        // when
        let got = source(peer(TARGET)).await;
        // then
        assert!(got.is_empty(), "a retracted advert was offered: {got:?}");
    }

    #[tokio::test]
    async fn target_is_not_offered_by_the_source() {
        // given
        let fx = setup(&[TARGET, 5]);
        let replica = replica_of(ME);
        advertise(&replica, TARGET, 7);
        advertise(&replica, 5, 8);
        let source = relay_source(fx.members.clone(), replica, peer(ME));
        // when
        let got = source(peer(TARGET)).await;
        // then
        assert_eq!(got, vec![candidate(5, 8)]);
    }
}

// --- setup and helpers ---

struct Fixture {
    members: Members,
}

impl Fixture {
    /// The candidates for a dial to `TARGET` from the adverts `(author seed, value)`.
    fn candidates(&self, adverts: &[(u8, Vec<u8>)]) -> Vec<RelayCandidate> {
        let map: HashMap<PeerId, Bytes> = adverts
            .iter()
            .map(|(seed, value)| (peer(*seed), Bytes::from(value.clone())))
            .collect();
        relay_candidates(&self.members, &map, peer(ME), peer(TARGET))
    }
}

/// Members with a verified connection to each of `connected`; the list admits all of them.
fn setup(connected: &[u8]) -> Fixture {
    setup_admitting(connected, connected)
}

/// Members where the list admits `admitted` and the verified connections are to `connected`.
fn setup_admitting(admitted: &[u8], connected: &[u8]) -> Fixture {
    let admitted: BTreeSet<PeerId> = admitted.iter().map(|s| peer(*s)).collect();
    let members = Members::with_access(view_admitting(admitted));
    for seed in connected {
        members.add_verified_conn(peer(*seed));
    }
    Fixture { members }
}

/// A view whose list admits `admitted` and the caller.
fn view_admitting(mut admitted: BTreeSet<PeerId>) -> AccessView {
    admitted.insert(peer(ME));
    let mut ledger = Ledger::empty();
    ledger.admitted = admitted;
    let view = AccessView::open();
    view.publish(Verdicts {
        me: peer(ME),
        ledger: Arc::new(ledger),
        blocks: BTreeSet::new(),
    });
    view
}

fn peer(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

/// The key `[seed; 32]`.
fn key(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn candidate(member: u8, relay: u8) -> RelayCandidate {
    RelayCandidate {
        member: peer(member),
        relay: key(relay),
        address: None,
    }
}

/// An empty in-memory replica of the network, owned by the node with the seed `owner`.
fn replica_of(owner: u8) -> Arc<MemReplica> {
    let ctx = SyncCtx {
        secret: secret(owner),
        network_seed: [9; 32],
        store_dir: None,
    };
    MemSyncNet::new()
        .backend(ctx)
        .replica_mem(REPLICA_NET)
        .expect("replica")
}

/// The advert of the member `author`: its relay key is `[relay; 32]`.
fn advertise(replica: &MemReplica, author: u8, relay: u8) {
    replica.put_raw(entry(author, &key(relay)));
}

fn entry(author: u8, value: &[u8]) -> ReplicaEntry {
    ReplicaEntry {
        author: peer(author),
        key: RELAY_ADVERT_KEY.to_vec(),
        value: Bytes::copy_from_slice(value),
        ts_micros: 1_000,
    }
}
