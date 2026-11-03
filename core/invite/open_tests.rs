//! Tests of the open-invite list (`open_of`) and of the note edit (`Ledger::relabel`).
//!
//! The pure list works on hand-made records; the edit runs on a ledger over an in-memory replica
//! where the records of other authors are put by hand.

use std::collections::BTreeSet;
use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::invite::proof::issue_note;
use crate::invite::store::UsedRecord;
use crate::invite::store_admins::issue_value;
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};

const NOW: u64 = 1_790_000_000;
const H1: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const H2: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

fn entry(author: PeerId, prefix: &str, hash: &str, value: Vec<u8>) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: format!("{prefix}{hash}").into_bytes(),
        value: Bytes::from(value),
        ts_micros: (NOW - 20) * 1_000_000,
    }
}

/// An issue record written by `author`; `issuer` makes it v2.
fn issue_by(
    author: PeerId,
    hash: &str,
    made: u64,
    who: Option<&str>,
    issuer: Option<PeerId>,
) -> ReplicaEntry {
    let value = issue_value(&issue_record(made, who), issuer).unwrap();
    entry(author, ISSUE_PREFIX, hash, value)
}

fn used_by(author: PeerId, hash: &str) -> ReplicaEntry {
    let rec = UsedRecord {
        v: 1,
        used_unix: NOW - 5,
    };
    entry(
        author,
        USED_PREFIX,
        hash,
        postcard::to_stdvec(&rec).unwrap(),
    )
}

mod open_list {
    use super::*;

    #[test]
    fn fresh_unused_invite_is_open() {
        // given
        let issued = [issue_by(pid(2), H1, NOW - 10, Some("Anna"), None)];
        // when
        let rows = open_of(&issued, &[], None, NOW);
        // then
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].for_whom.as_deref(), Some("Anna"));
        assert_eq!(rows[0].made_unix, NOW - 10);
    }

    #[test]
    fn used_invite_is_not_open() {
        // given
        let issued = [issue_by(pid(2), H1, NOW - 10, None, None)];
        let used = [used_by(pid(2), H1)];
        // when / then
        assert!(open_of(&issued, &used, None, NOW).is_empty());
    }

    #[test]
    fn expired_invite_is_not_open() {
        // given: the record is exactly one lifetime old
        let issued = [issue_by(pid(2), H1, NOW - INVITE_TTL_SECS, None, None)];
        // when / then
        assert!(open_of(&issued, &[], None, NOW).is_empty());
    }

    #[test]
    fn holder_copy_does_not_duplicate_the_row() {
        // given: the issuer's record with a note and a holder's copy without one
        let issued = [
            issue_by(pid(2), H1, NOW - 10, Some("Anna"), Some(pid(2))),
            issue_by(pid(3), H1, NOW - 10, None, Some(pid(2))),
        ];
        // when
        let rows = open_of(&issued, &[], None, NOW);
        // then
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].for_whom.as_deref(), Some("Anna"));
        assert_eq!(rows[0].issuer, Some(pid(2)));
    }

    #[test]
    fn newest_first() {
        // given
        let issued = [
            issue_by(pid(2), H1, NOW - 100, None, None),
            issue_by(pid(2), H2, NOW - 10, None, None),
        ];
        // when
        let rows = open_of(&issued, &[], None, NOW);
        // then
        let hashes: Vec<&str> = rows.iter().map(|r| r.hash.as_str()).collect();
        assert_eq!(hashes, [H2, H1]);
    }

    #[test]
    fn managed_non_admin_record_is_not_open() {
        // given: the author is not among the admins
        let issued = [issue_by(pid(5), H1, NOW - 10, None, Some(pid(5)))];
        let admins = BTreeSet::from([pid(2)]);
        // when / then
        assert!(open_of(&issued, &[], Some(&admins), NOW).is_empty());
    }
}

mod managed_fields {
    use super::*;

    #[test]
    fn managed_fields_come_from_admin_records() {
        // given: a plain member writes a newer record of its own under the admin's hash
        let issued = [
            issue_by(pid(5), H1, NOW - 1, Some("Spoof"), Some(pid(5))),
            issue_by(pid(2), H1, NOW - 100, Some("Anna"), Some(pid(2))),
        ];
        let admins = BTreeSet::from([pid(2)]);
        // when
        let rows = open_of(&issued, &[], Some(&admins), NOW);
        // then: note, time and issuer are the admin's
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].for_whom.as_deref(), Some("Anna"));
        assert_eq!(rows[0].made_unix, NOW - 100);
        assert_eq!(rows[0].issuer, Some(pid(2)));
    }
}

mod relabel {
    use super::*;

    #[tokio::test]
    async fn issuer_rewrites_label_keeping_made_unix_and_issuer() {
        // given: our own v2 record and a holder's copy next to it
        let rig = setup().await;
        rig.given_own(H1, NOW - 10, Some("Old"));
        rig.given(issue_by(pid(3), H1, NOW - 10, None, Some(rig.me)));
        // when
        let got = rig.ledger.relabel(H1, Some("Ann"), NOW, rig.admins()).await;
        // then
        assert_eq!(got.unwrap(), Relabel::Done);
        let own = rig.own_record(H1).await;
        assert_eq!(issuer_of(&own.value), Some(rig.me));
        let rec: IssueRecord = postcard::from_bytes(&own.value).unwrap();
        assert_eq!((rec.v, rec.made_unix), (2, NOW - 10));
        assert_eq!(issue_note(&rig.records(H1).await), "Ann");
        let verdict = rig.ledger.verdict(H1, NOW, rig.admins()).await;
        assert_eq!(verdict, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn empty_label_clears_for_whom() {
        // given
        let rig = setup().await;
        rig.given_own(H1, NOW - 10, Some("Old"));
        // when
        let got = rig.ledger.relabel(H1, None, NOW, rig.admins()).await;
        // then
        assert_eq!(got.unwrap(), Relabel::Done);
        let rows = rig.ledger.open(NOW, rig.admins()).await.unwrap();
        assert_eq!(rows[0].for_whom, None);
    }

    #[tokio::test]
    async fn someone_elses_invite_is_not_issuer() {
        // given: another admin's record
        let rig = setup().await;
        let theirs = issue_by(pid(3), H1, NOW - 10, Some("Theirs"), Some(pid(3)));
        rig.given(theirs.clone());
        // when
        let got = rig
            .ledger
            .relabel(H1, Some("Mine"), NOW, rig.admins())
            .await;
        // then
        assert_eq!(got.unwrap(), Relabel::NotIssuer);
        assert_eq!(rig.records(H1).await, [theirs]);
    }

    #[tokio::test]
    async fn holder_copy_is_not_relabelled() {
        // given: we only hold a copy of the other admin's invite
        let rig = setup().await;
        rig.given(issue_by(pid(3), H1, NOW - 10, Some("Theirs"), Some(pid(3))));
        let copy = issue_value(&issue_record(NOW - 10, None), Some(pid(3))).unwrap();
        rig.put_own(H1, copy);
        // when
        let got = rig
            .ledger
            .relabel(H1, Some("Mine"), NOW, rig.admins())
            .await;
        // then
        assert_eq!(got.unwrap(), Relabel::NotIssuer);
        let rec: IssueRecord = postcard::from_bytes(&rig.own_record(H1).await.value).unwrap();
        assert_eq!(rec.for_whom, None);
    }

    #[tokio::test]
    async fn used_invite_is_not_found() {
        // given
        let rig = setup().await;
        rig.given_own(H1, NOW - 10, Some("Old"));
        rig.given(used_by(rig.me, H1));
        let before = rig.own_record(H1).await;
        // when
        let got = rig.ledger.relabel(H1, Some("New"), NOW, rig.admins()).await;
        // then
        assert_eq!(got.unwrap(), Relabel::NotFound);
        assert_eq!(rig.own_record(H1).await.value, before.value);
    }

    #[tokio::test]
    async fn own_made_unix_kept_when_copy_is_newer() {
        // given: our record is older than a holder's copy
        let rig = setup().await;
        rig.given_own(H1, NOW - 100, Some("Old"));
        rig.given(issue_by(pid(3), H1, NOW - 10, None, Some(rig.me)));
        // when
        let got = rig.ledger.relabel(H1, Some("Ann"), NOW, rig.admins()).await;
        // then: the lifetime is not extended
        assert_eq!(got.unwrap(), Relabel::Done);
        let rec: IssueRecord = postcard::from_bytes(&rig.own_record(H1).await.value).unwrap();
        assert_eq!(rec.made_unix, NOW - 100);
    }

    #[tokio::test]
    async fn own_expired_with_fresh_copy_is_not_found() {
        // given: our record is expired, a holder's copy is fresh
        let rig = setup().await;
        rig.given_own(H1, NOW - INVITE_TTL_SECS, Some("Old"));
        rig.given(issue_by(pid(3), H1, NOW - 10, None, Some(rig.me)));
        let before = rig.own_record(H1).await;
        // when
        let got = rig.ledger.relabel(H1, Some("Ann"), NOW, rig.admins()).await;
        // then
        assert_eq!(got.unwrap(), Relabel::NotFound);
        assert_eq!(rig.own_record(H1).await.value, before.value);
    }

    #[tokio::test]
    async fn expired_invite_is_not_found() {
        // given
        let rig = setup().await;
        rig.given_own(H1, NOW - INVITE_TTL_SECS, Some("Old"));
        // when
        let got = rig.ledger.relabel(H1, Some("New"), NOW, rig.admins()).await;
        // then
        assert_eq!(got.unwrap(), Relabel::NotFound);
    }
}

/// A ledger of node `me` over an in-memory replica where foreign records are put by hand.
struct Rig {
    ledger: Ledger,
    replica: Arc<MemReplica>,
    me: PeerId,
    admins: BTreeSet<PeerId>,
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
    let ledger = Ledger::of(&list);
    let me = ledger.me();
    Rig {
        ledger,
        replica: shared,
        me,
        admins: BTreeSet::from([me, pid(3)]),
        _keep: (net, backend),
    }
}

impl Rig {
    fn admins(&self) -> Option<&BTreeSet<PeerId>> {
        Some(&self.admins)
    }

    fn given(&self, e: ReplicaEntry) {
        self.replica.put_raw(e);
    }

    /// Our own record, written the way the console does it.
    fn given_own(&self, hash: &str, made: u64, who: Option<&str>) {
        self.given(issue_by(self.me, hash, made, who, Some(self.me)));
    }

    fn put_own(&self, hash: &str, value: Vec<u8>) {
        self.given(entry(self.me, ISSUE_PREFIX, hash, value));
    }

    async fn records(&self, hash: &str) -> Vec<ReplicaEntry> {
        let mut all = self.ledger.exact(issue_key(hash)).await.unwrap();
        all.sort_by_key(|e| e.author.as_bytes().to_vec());
        all
    }

    async fn own_record(&self, hash: &str) -> ReplicaEntry {
        let mine = self.records(hash).await;
        mine.into_iter()
            .find(|e| e.author == self.me)
            .expect("our own record")
    }
}
