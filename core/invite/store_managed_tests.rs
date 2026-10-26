//! Tests of the invite verdict under the rules of a Managed network (`Ledger::verdict` with a set of admins):
//! only the records written by administrators count; with no set of admins yet every author counts.
//! The issue records themselves (issuer, holder copies, old formats) are in `store_managed_issue_tests.rs`.
//!
//! The ledger sits on an in-memory replica where records of several authors are put by hand.
//!
//! assumed: this file is declared in `store.rs` as
//! `#[cfg(test)] #[path = "store_managed_tests.rs"] mod managed_tests;`.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::invite::store_admins::issue_value;
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};

const NOW: u64 = 1_790_000_000;
const TTL: u64 = INVITE_TTL_SECS;
const HASH: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[path = "store_managed_issue_tests.rs"]
mod issue_records;

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

mod no_admin_list {
    use super::*;

    #[tokio::test]
    async fn issue_of_any_author_grants() {
        // given: no admin set yet; a stranger writes a plain record
        let rig = setup().await;
        rig.given_v1_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_without_admins().await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn used_mark_of_any_author_is_used() {
        // given
        let rig = setup().await;
        rig.given_v1_issue_by(stranger(), NOW - 10);
        rig.given_used_by(stranger());
        // when
        let got = rig.when_judged_without_admins().await;
        // then
        assert_eq!(got, IssueVerdict::Used);
    }

    #[tokio::test]
    async fn absent_is_unknown() {
        // given
        let rig = setup().await;
        // when
        let got = rig.when_judged_without_admins().await;
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
    /// The author's own record of a Managed network: v2, the issuer is the author.
    fn given_issue_by(&self, author: PeerId, made_unix: u64) {
        self.given_copy_by(author, author, made_unix);
    }

    /// A record `holder` wrote on behalf of `issuer` (a holder's copy, or the issuer's own when they are equal).
    fn given_copy_by(&self, holder: PeerId, issuer: PeerId, made_unix: u64) {
        let value = issue_value(&issue_record(made_unix, None), Some(issuer)).unwrap();
        self.put(holder, ISSUE_PREFIX, value);
    }

    /// A plain record: v1, no issuer in it.
    fn given_v1_issue_by(&self, author: PeerId, made_unix: u64) {
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

    async fn when_judged_without_admins(&self) -> IssueVerdict {
        self.ledger.verdict(HASH, NOW, None).await
    }
}

fn admin() -> PeerId {
    pid(11)
}

fn stranger() -> PeerId {
    pid(12)
}

/// A second admin, or a former one.
fn third() -> PeerId {
    pid(13)
}
