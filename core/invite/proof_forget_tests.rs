//! Tests of `admit_owner` with the `after` mark: the admit record of an owner who comes back after being
//! forgotten (`proof.rs`).
//!
//! A returning owner already has a live admit of this node under the same key. With `after = Some(t)`, where
//! `t` is the time of the last vote that forgot the owner, the old record must be replaced by one that is
//! strictly later than `t`, so that it counts as a return. With `None` the first live record stays.
//! The full server path is in `server_owner_forget_tests.rs`.

use std::sync::Arc;

use super::admit_owner;
use crate::invite::store::{issue_record, Ledger};
use crate::invite::InviteSecret;
use crate::members::access::record::{admit_key, parse_entry, AclKind, AclValue, Parsed, Via};
use crate::proxy::domains::DomainList;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{PeerId, Replica};

const NOW: u64 = 1_792_400_000;

mod with_mark {
    use super::*;

    #[tokio::test]
    async fn clock_behind_the_mark_gives_made_unix_after_the_mark() {
        // given: an owner admitted at NOW and forgotten by a vote far in the future of this node's clock
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        let mark = NOW + 500;
        // when: the owner is admitted again with the mark, the clock still behind it
        node.when_admitted_after(Some(mark), NOW + 10).await;
        // then: the new record is one second later than the mark
        assert_eq!(node.admit_time().await, Some(mark + 1));
    }

    #[tokio::test]
    async fn clock_equal_to_the_mark_gives_made_unix_after_the_mark() {
        // given: an owner admitted and forgotten at the very second of the clock
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        // when
        node.when_admitted_after(Some(NOW + 10), NOW + 10).await;
        // then: the record is not in the same second as the mark, which would not count as a return
        assert_eq!(node.admit_time().await, Some(NOW + 11));
    }

    #[tokio::test]
    async fn mark_at_the_largest_time_does_not_overflow() {
        // given: an owner admitted before and a forget vote stamped with the largest possible time
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        // when: the owner is admitted again after that mark
        node.when_admitted_after(Some(u64::MAX), NOW + 10).await;
        // then: the stamp saturates instead of wrapping
        assert_eq!(node.admit_time().await, Some(u64::MAX));
    }

    #[tokio::test]
    async fn clock_ahead_of_the_mark_gives_the_clock() {
        // given: an owner admitted and forgotten long ago
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        // when: admitted again, the clock well past the mark
        node.when_admitted_after(Some(NOW + 5), NOW + 100).await;
        // then: the record carries the current time
        assert_eq!(node.admit_time().await, Some(NOW + 100));
    }

    #[tokio::test]
    async fn without_an_old_record_the_admit_is_still_written() {
        // given: a node that has no admit of the owner
        let node = setup().await;
        // when: the owner comes with a mark
        node.when_admitted_after(Some(NOW + 500), NOW).await;
        // then: the record is written, later than the mark
        assert_eq!(node.admit_time().await, Some(NOW + 501));
    }

    #[tokio::test]
    async fn rewrite_keeps_the_invite_origin_and_the_issue_note() {
        // given: an invite labelled "Masha" and an owner already admitted through it
        let node = setup().await;
        node.when_issued("Masha").await;
        node.given_admitted_at(NOW).await;
        // when: the owner is admitted again after being forgotten
        node.when_admitted_after(Some(NOW + 500), NOW + 10).await;
        // then: the new record is an admit by invite with the same label
        let want = AclValue {
            kind: AclKind::Admit,
            subject: pid(7),
            via: Via::Invite,
            made_unix: NOW + 501,
            note: "Masha".into(),
        };
        assert_eq!(node.admit().await, Some(want));
    }

    #[tokio::test]
    async fn rewrite_leaves_one_live_record() {
        // given: an owner already admitted
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        // when: admitted again with a mark, twice (a retry)
        node.when_admitted_after(Some(NOW + 500), NOW + 10).await;
        node.when_admitted_after(Some(NOW + 500), NOW + 20).await;
        // then: still one live admit under the key
        assert_eq!(node.live_admits().await, 1);
    }
}

mod without_mark {
    use super::*;

    #[tokio::test]
    async fn none_keeps_the_first_record() {
        // given: an owner admitted at NOW
        let node = setup().await;
        node.given_admitted_at(NOW).await;
        // when: the same owner is admitted again later, with no mark
        node.when_admitted_after(None, NOW + 3).await;
        // then: the first record stays, with its time
        assert_eq!(node.admit_time().await, Some(NOW));
        assert_eq!(node.live_admits().await, 1);
    }

    #[tokio::test]
    async fn none_writes_the_clock_time_when_there_is_no_record() {
        // given: a node that has no admit of the owner
        let node = setup().await;
        // when: admitted with no mark
        node.when_admitted_after(None, NOW).await;
        // then: the record carries the current time
        assert_eq!(node.admit_time().await, Some(NOW));
    }
}

fn invite(seed: u8) -> InviteSecret {
    InviteSecret::from_bytes([seed; 32])
}

/// A node with its in-memory shared replica and a domain list over it; the owner is always `pid(7)`.
struct Node {
    list: DomainList,
    me: PeerId,
    shared: Arc<dyn Replica>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

async fn setup() -> Node {
    let net = MemSyncNet::new();
    let me = secret(3);
    let backend = net.backend(SyncCtx {
        secret: me.clone(),
        network_seed: [9; 32],
        store_dir: None,
    });
    let shared: Arc<dyn Replica> = backend.replica_mem(REPLICA_NET).unwrap();
    let own: Arc<dyn Replica> = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    Node {
        list,
        me: me.peer_id(),
        shared,
        _keep: (net, backend),
    }
}

impl Node {
    /// The owner already holds this node's admit record made at `made_unix` (no mark).
    async fn given_admitted_at(&self, made_unix: u64) {
        self.when_admitted_after(None, made_unix).await;
    }

    async fn when_admitted_after(&self, after: Option<u64>, now_unix: u64) {
        let hash = invite(1).hash_hex();
        admit_owner(&self.list, &hash, pid(7), after, now_unix)
            .await
            .unwrap();
    }

    async fn when_issued(&self, for_whom: &str) {
        let rec = issue_record(NOW, Some(for_whom));
        Ledger::of(&self.list)
            .issue(&invite(1).hash_hex(), &rec)
            .await
            .unwrap();
    }

    /// The admit this node wrote for the owner.
    async fn admit(&self) -> Option<AclValue> {
        let entry = self
            .shared
            .get(self.me, &admit_key(&pid(7)))
            .await
            .unwrap()?;
        match parse_entry(&entry) {
            Parsed::Record { author, value, .. } if author == self.me => Some(value),
            other => panic!("the entry under the admit key is not this node's record: {other:?}"),
        }
    }

    async fn admit_time(&self) -> Option<u64> {
        self.admit().await.map(|v| v.made_unix)
    }

    async fn live_admits(&self) -> usize {
        let all = self.shared.scan(b"acl/admit/").await.unwrap();
        all.iter().filter(|e| !e.is_tombstone()).count()
    }
}
