//! Tests of the invite verdict under the rules of a Managed network (`Ledger::verdict` with a set of admins):
//! only the records written by administrators count; in a Peer network (no set) every author counts.
//!
//! The ledger sits on an in-memory replica where records of several authors are put by hand.
//!
//! assumed: this file is declared in `store.rs` as
//! `#[cfg(test)] #[path = "store_managed_tests.rs"] mod managed_tests;`.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};

const NOW: u64 = 1_790_000_000;
const TTL: u64 = INVITE_TTL_SECS;
const HASH: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

mod issue_records {
    use super::*;

    #[tokio::test]
    async fn only_from_a_stranger_is_not_admin() {
        // given: the only issue record was written by a node that is not an admin
        let rig = setup().await;
        rig.given_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }

    #[tokio::test]
    async fn from_an_admin_grants() {
        // given
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn from_a_stranger_and_an_admin_grants() {
        // given: the stranger's record is ignored, the admin's one is enough
        let rig = setup().await;
        rig.given_issue_by(stranger(), NOW - 10);
        rig.given_issue_by(admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn fresh_one_of_a_stranger_does_not_save_an_expired_one_of_an_admin() {
        // given: the admin issued long ago, the stranger "issued" just now
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - TTL - 1);
        rig.given_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Expired);
    }

    #[tokio::test]
    async fn from_an_author_outside_an_empty_admin_set_is_not_admin() {
        // given
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        // when: nobody is an admin at this node's view
        let got = rig.when_judged_among(&[]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }

    #[tokio::test]
    async fn absent_is_unknown() {
        // given: no records at all
        let rig = setup().await;
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Unknown);
    }
}

mod used_marks {
    use super::*;

    #[tokio::test]
    async fn from_an_admin_is_used() {
        // given
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        rig.given_used_by(admin());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Used);
    }

    #[tokio::test]
    async fn from_a_stranger_is_not_used() {
        // given: a stranger burns the invite of an admin
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        rig.given_used_by(stranger());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(
            got,
            IssueVerdict::Grant,
            "a stranger's mark must not burn the invite"
        );
    }

    #[tokio::test]
    async fn from_a_stranger_without_any_issue_is_unknown() {
        // given
        let rig = setup().await;
        rig.given_used_by(stranger());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Unknown);
    }

    #[tokio::test]
    async fn next_to_an_issue_of_a_stranger_stays_not_admin() {
        // given: the stranger wrote both the issue and the mark
        let rig = setup().await;
        rig.given_issue_by(stranger(), NOW - 10);
        rig.given_used_by(stranger());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }
}

mod claim_records {
    use super::*;

    #[tokio::test]
    async fn from_a_stranger_is_not_used() {
        // given: an admin's issue and a claim of a stranger
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        rig.given_claim_by(stranger());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }
}

mod peer_rules {
    use super::*;

    #[tokio::test]
    async fn issue_of_any_author_grants() {
        // given: no admin set (a Peer network)
        let rig = setup().await;
        rig.given_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_as_peer().await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn used_mark_of_any_author_is_used() {
        // given
        let rig = setup().await;
        rig.given_issue_by(stranger(), NOW - 10);
        rig.given_used_by(stranger());
        // when
        let got = rig.when_judged_as_peer().await;
        // then
        assert_eq!(got, IssueVerdict::Used);
    }

    #[tokio::test]
    async fn absent_is_unknown() {
        // given
        let rig = setup().await;
        // when
        let got = rig.when_judged_as_peer().await;
        // then
        assert_eq!(got, IssueVerdict::Unknown);
    }
}

/// A ledger over an in-memory replica that foreign records are put into.
struct Rig {
    ledger: Ledger,
    replica: Arc<MemReplica>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

async fn setup() -> Rig {
    let net = MemSyncNet::new();
    let backend = net.backend(SyncCtx {
        secret: secret(2),
        network_seed: [9; 32],
        store_dir: None,
    });
    let shared = backend.replica_mem(REPLICA_NET).unwrap();
    let own = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    Rig {
        ledger: Ledger::of(&list),
        replica: shared,
        _keep: (net, backend),
    }
}

impl Rig {
    fn given_issue_by(&self, author: PeerId, made_unix: u64) {
        let value = postcard::to_stdvec(&issue_record(made_unix, None)).unwrap();
        self.put(author, ISSUE_PREFIX, value);
    }

    fn given_used_by(&self, author: PeerId) {
        let value = postcard::to_stdvec(&UsedRecord {
            v: 1,
            used_unix: NOW - 5,
        })
        .unwrap();
        self.put(author, USED_PREFIX, value);
    }

    fn given_claim_by(&self, author: PeerId) {
        let value = postcard::to_stdvec(&ClaimRecord {
            v: 1,
            claimed_unix: NOW - 5,
        })
        .unwrap();
        self.put(author, CLAIM_PREFIX, value);
    }

    fn put(&self, author: PeerId, prefix: &str, value: Vec<u8>) {
        self.replica.put_raw(ReplicaEntry {
            author,
            key: format!("{prefix}{HASH}").into_bytes(),
            value: Bytes::from(value),
            ts_micros: (NOW - 20) * 1_000_000,
        });
    }

    async fn when_judged_among(&self, admins: &[PeerId]) -> IssueVerdict {
        let set: BTreeSet<PeerId> = admins.iter().copied().collect();
        self.ledger.verdict(HASH, NOW, Some(&set)).await
    }

    async fn when_judged_as_peer(&self) -> IssueVerdict {
        self.ledger.verdict(HASH, NOW, None).await
    }
}

fn admin() -> PeerId {
    pid(11)
}

fn stranger() -> PeerId {
    pid(12)
}
