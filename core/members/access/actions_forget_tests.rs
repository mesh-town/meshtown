//! Tests of the forget and unforget actions (`actions.rs`): a vote to drop a node that has been off
//! for good, and taking that vote back. Each test runs against real in-memory replicas with the
//! verdicts recomputed by `Access`. Nodes: `me` (seed 1) and the members `a` (2) and `b` (3) are on
//! the list, so three nodes are active and two votes are needed; in a network of two one vote is enough.

use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, forget_key, key_for, parse_entry, Parsed};
use crate::members::access::tally::Ledger;
use crate::members::access::view::{AccessView, Deny};
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{Replica, ReplicaEntry};

/// A time (seconds) far ahead of any real clock.
const FUTURE: u64 = 4_000_000_000;

mod forget {
    use super::*;

    #[tokio::test]
    async fn forget_offline_member_writes_record() {
        // given: three active nodes, b is not connected
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        let before = unix_now();
        // when: I vote to forget b
        let changed = me.apply(MemberAction::Forget(b)).await;
        // then: my forget record about b is stored, stamped with the current time and no origin or note
        assert_eq!(changed, Ok(true));
        let value = me.my_forget(b).await.expect("my vote must be stored");
        assert_eq!(
            (value.kind, value.subject, value.via),
            (AclKind::Forget, b, Via::None)
        );
        assert_eq!(value.note, "");
        assert!(
            (before..=unix_now()).contains(&value.made_unix),
            "the vote must carry the current time"
        );
    }

    #[tokio::test]
    async fn first_vote_waits_for_a_second_and_hides_the_node_from_me() {
        // given / when: one of two needed votes
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        // then: the circle holds my vote only, b is not forgotten yet but denied for me
        let ledger = me.ledger();
        assert_eq!(ledger.forgets[&b].voters, vec![me.me]);
        assert_eq!(ledger.forgets[&b].at_unix, None);
        assert!(ledger.is_member(&b), "one vote of two forgets nobody");
        assert_eq!(me.verdict(b), Err(Deny::RemovedByMe));
    }

    #[tokio::test]
    async fn forget_twice_in_round_is_noop() {
        // given: my vote about b is in the current circle
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        let before = me.entries().await;
        // when: the same action again
        let again = me.apply(MemberAction::Forget(b)).await;
        // then: nothing is written, the record stays as it was
        assert_eq!(again, Ok(false));
        assert_eq!(me.entries().await, before, "the vote was rewritten");
    }

    #[tokio::test]
    async fn confirm_by_second_member_forgets() {
        // given: I voted to forget b and the record reached member a
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (me.peer(2, &[me.me, pid(3)]), pid(3));
        me.apply(MemberAction::Forget(b)).await.unwrap();
        a.pull_from(&me).await;
        // when: a confirms through its own node and its record comes back to me
        let changed = a.apply(MemberAction::Forget(b)).await;
        me.pull_from(&a).await;
        me.refresh().await;
        // then: b is forgotten by both votes (same second: the order is by id)
        assert_eq!(changed, Ok(true));
        let mut voters = me.ledger().forgets[&b].voters.clone();
        voters.sort();
        assert_eq!(voters, [me.me.min(a.me), me.me.max(a.me)]);
        assert!(me.ledger().is_forgotten(&b));
        assert_eq!(me.verdict(b), Err(Deny::Forgotten));
        assert_eq!(me.verdict(a.me), Ok(()), "a stays a member");
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn forget_online_member_is_refused() {
        // given: b is connected right now
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.go_online(b);
        let before = me.entries().await;
        // when: I vote to forget it
        let result = me.apply(MemberAction::Forget(b)).await;
        // then: refused, nothing is written
        assert_eq!(result, Err(ActionRefusal::Online));
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn forget_self_is_refused() {
        let me = setup(&[pid(2), pid(3)]);
        let result = me.apply(MemberAction::Forget(me.me)).await;
        assert_eq!(result, Err(ActionRefusal::SelfTarget));
        assert!(me.my_forget(me.me).await.is_none());
    }

    #[tokio::test]
    async fn forget_unlisted_is_not_member() {
        // given: a node that is not on the list
        let me = setup(&[pid(2)]);
        let before = me.entries().await;
        // when / then: refused, nothing is written
        let result = me.apply(MemberAction::Forget(pid(9))).await;
        assert_eq!(result, Err(ActionRefusal::NotMember));
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn forget_forgotten_is_not_member() {
        // given: a is forgotten
        let me = setup(&[pid(2)]);
        let a = pid(2);
        me.apply(MemberAction::Forget(a)).await.unwrap();
        assert!(me.ledger().is_forgotten(&a));
        // when / then: it is no member to vote about
        assert_eq!(
            me.apply(MemberAction::Forget(a)).await,
            Err(ActionRefusal::NotMember)
        );
    }

    #[tokio::test]
    async fn left_node_cannot_forget() {
        // given: I have left the network
        let me = setup(&[pid(2), pid(3)]);
        me.apply(MemberAction::Leave).await.unwrap();
        let before = me.entries().await;
        // when / then: neither the vote nor taking it back is allowed, nothing is written
        assert_eq!(
            me.apply(MemberAction::Forget(pid(3))).await,
            Err(ActionRefusal::Left)
        );
        assert_eq!(
            me.apply(MemberAction::Unforget(pid(3))).await,
            Err(ActionRefusal::Left)
        );
        assert_eq!(me.entries().await, before);
    }
}

mod forget_again {
    use super::*;

    #[tokio::test]
    async fn forget_again_after_return_rewrites() {
        // given: a was forgotten and then admitted again one second later
        let me = setup(&[pid(2)]);
        let a = pid(2);
        me.apply(MemberAction::Forget(a)).await.unwrap();
        let first = me.my_forget(a).await.unwrap().made_unix;
        me.given_admit(me.me, a, first + 1, 2);
        me.refresh().await;
        assert_eq!(me.verdict(a), Ok(()), "the admit must bring a back");
        // when: I vote to forget it again
        let changed = me.apply(MemberAction::Forget(a)).await;
        // then: the record is rewritten with a time after the admit and a is forgotten again
        assert_eq!(changed, Ok(true));
        assert!(me.my_forget(a).await.unwrap().made_unix > first + 1);
        assert_eq!(me.verdict(a), Err(Deny::Forgotten));
    }

    #[tokio::test]
    async fn forget_again_steps_over_future_admit() {
        // given: the latest admit of b is stamped far ahead of my clock
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.given_admit(me.me, b, FUTURE, 2);
        me.refresh().await;
        // when: I vote to forget b
        let changed = me.apply(MemberAction::Forget(b)).await;
        // then: the vote is stamped one second after that admit, or it would be older than the admit
        assert_eq!(changed, Ok(true));
        assert_eq!(me.my_forget(b).await.unwrap().made_unix, FUTURE + 1);
        assert_eq!(me.verdict(b), Err(Deny::RemovedByMe));
    }
}

mod unforget {
    use super::*;

    #[tokio::test]
    async fn unforget_withdraws_own_record() {
        // given: my vote to forget b
        let me = setup(&[pid(2), pid(3)]);
        let b = pid(3);
        me.apply(MemberAction::Forget(b)).await.unwrap();
        // when: I take it back
        let changed = me.apply(MemberAction::Unforget(b)).await;
        // then: the record is a tombstone, the circle is gone and b passes again
        assert_eq!(changed, Ok(true));
        let entries = me.shared.scan(&forget_key(&b)).await.unwrap();
        let entry = entries.into_iter().find(|e| e.author == me.me);
        assert!(
            entry.expect("the key must keep a tombstone").is_tombstone(),
            "the vote must be withdrawn, not deleted"
        );
        assert!(!me.ledger().forgets.contains_key(&b));
        assert_eq!(me.verdict(b), Ok(()));
    }

    #[tokio::test]
    async fn unforget_without_record_is_not_forgotten() {
        let me = setup(&[pid(2), pid(3)]);
        // when / then: there is nothing to take back, nothing is written
        let before = me.entries().await;
        assert_eq!(
            me.apply(MemberAction::Unforget(pid(3))).await,
            Err(ActionRefusal::NotForgotten)
        );
        assert_eq!(me.entries().await, before);
    }

    #[tokio::test]
    async fn unforget_does_not_touch_the_votes_of_others() {
        // given: only a has voted to forget b
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (pid(2), pid(3));
        me.given(a, AclKind::Forget, b, 200, 2);
        me.refresh().await;
        // when / then: my take-back is refused, a's vote stays in the circle
        assert_eq!(
            me.apply(MemberAction::Unforget(b)).await,
            Err(ActionRefusal::NotForgotten)
        );
        assert_eq!(me.ledger().forgets[&b].voters, vec![a]);
    }

    #[tokio::test]
    async fn unforget_one_of_two_votes_returns_member() {
        // given: b is forgotten by my vote and the vote of a
        let me = setup(&[pid(2), pid(3)]);
        let (a, b) = (me.peer(2, &[me.me, pid(3)]), pid(3));
        me.apply(MemberAction::Forget(b)).await.unwrap();
        a.pull_from(&me).await;
        a.apply(MemberAction::Forget(b)).await.unwrap();
        me.pull_from(&a).await;
        me.refresh().await;
        assert_eq!(me.verdict(b), Err(Deny::Forgotten));
        // when: I take my vote back
        let changed = me.apply(MemberAction::Unforget(b)).await;
        // then: one vote is below the threshold, b is a member again and a's vote waits
        assert_eq!(changed, Ok(true));
        assert_eq!(me.verdict(b), Ok(()));
        assert_eq!(me.ledger().forgets[&b].voters, vec![a.me]);
        assert_eq!(me.ledger().forgets[&b].at_unix, None);
    }
}

// --- setup and helpers ---

/// A node of the in-memory network with its replicas.
struct Node {
    access: Access,
    shared: Arc<MemReplica>,
    me: PeerId,
    net: Arc<MemSyncNet>,
    _backend: Arc<MemSync>,
}

/// The node `me` (seed 1) with `listed` admitted by it; the view starts pending, as in a real start.
fn setup(listed: &[PeerId]) -> Node {
    let node = Node::new(&MemSyncNet::new(), 1);
    listed
        .iter()
        .for_each(|p| node.given_admit(node.me, *p, 111, 1));
    node
}

impl Node {
    fn new(net: &Arc<MemSyncNet>, seed: u8) -> Node {
        let secret = secret(seed);
        let ctx = SyncCtx {
            secret: secret.clone(),
            network_seed: [9; 32],
            store_dir: None,
        };
        let backend = net.backend(ctx);
        let (shared, own) = (
            backend.replica_mem(REPLICA_NET).unwrap(),
            backend.replica_mem(REPLICA_DOMAINS).unwrap(),
        );
        let members = Members::with_access(AccessView::pending());
        let access = Access::from_parts(secret.peer_id(), shared.clone(), own, members);
        Node {
            access,
            shared,
            me: secret.peer_id(),
            net: net.clone(),
            _backend: backend,
        }
    }

    /// Another node of the same network with its own replicas; it admitted `listed`.
    fn peer(&self, seed: u8, listed: &[PeerId]) -> Node {
        let node = Node::new(&self.net, seed);
        listed
            .iter()
            .for_each(|p| node.given_admit(node.me, *p, 111, 1));
        node
    }

    fn given_admit(&self, author: PeerId, subject: PeerId, made_unix: u64, ts_micros: u64) {
        self.given(author, AclKind::Admit, subject, made_unix, ts_micros);
    }

    fn given(&self, author: PeerId, kind: AclKind, subject: PeerId, made_unix: u64, ts: u64) {
        let value = AclValue {
            kind,
            subject,
            via: Via::Invite,
            made_unix,
            note: String::new(),
        };
        self.shared.put_raw(ReplicaEntry {
            author,
            key: key_for(kind, &subject),
            value: Bytes::from(encode(&value)),
            ts_micros: ts,
        });
    }

    /// The records of `other` reach this node (the sync that the real replicas do).
    async fn pull_from(&self, other: &Node) {
        for entry in other.shared.scan(b"").await.unwrap() {
            self.shared.put_raw(entry);
        }
    }

    fn go_online(&self, peer: PeerId) {
        self.access.members.add_verified_conn(peer);
    }

    async fn apply(&self, act: MemberAction) -> Result<bool, ActionRefusal> {
        apply(&self.access, act).await
    }

    async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }

    fn verdict(&self, peer: PeerId) -> Result<(), Deny> {
        self.access.view.check(peer)
    }

    fn ledger(&self) -> Arc<Ledger> {
        self.access
            .view
            .verdicts()
            .expect("the view must be published after an action")
            .ledger
            .clone()
    }

    /// My live forget record about `subject`, if any.
    async fn my_forget(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self
            .shared
            .get(self.me, &forget_key(&subject))
            .await
            .unwrap();
        match parse_entry(&entry.filter(|e| !e.is_tombstone())?) {
            Parsed::Record { value, .. } => Some(value),
            other => panic!("the forget vote must be a record, got {other:?}"),
        }
    }

    async fn entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }
}
