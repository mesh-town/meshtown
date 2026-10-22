//! Tests of the member actions (`actions.rs`): exclude, unexclude, block, unblock and leave, each against a
//! real in-memory shared replica and an own replica, with the verdicts recomputed by `Access`.
//! Nodes: `me` is the own node (seed 1), `a`, `b`, `c` are listed by me, `stranger` is not on the list.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;

use crate::access::actions::{apply, ActionRefusal, MemberAction};
use crate::access::record::{admit_key, block_key, encode, exclude_key, parse_entry, AclKind, AclValue, Parsed, Via};
use crate::access::runtime::Access;
use crate::access::tally::{How, Ledger};
use crate::access::view::{AccessView, Deny};
use crate::membership::Members;
use crate::net::contract::{pid, secret};
use crate::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::net::{
    BoxFuture, NetError, NetResult, PeerId, Replica, ReplicaEntry, ReplicaEvent, ReplicaId,
};

mod exclude {
    use super::*;

    #[tokio::test]
    async fn exclude_writes_vote_and_denies_locally() {
        // given: me, a and b on the list: two votes are needed to exclude
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        // when: I vote to exclude a
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: my vote is in the shared replica, it works at once for me, but a is not excluded
        assert_eq!(changed, Ok(true));
        assert_eq!(world.vote_subjects().await, vec![a], "my votes");
        assert_eq!(world.verdict(a), Err(Deny::VotedByMe));
        assert_eq!(world.ledger().pending[&a].voters, vec![world.me], "the proposal is mine");
        assert!(!world.ledger().excluded.contains_key(&a), "one vote is not enough yet");
        assert_eq!(world.verdict(pid(3)), Ok(()), "the other neighbour is untouched");
    }

    #[tokio::test]
    async fn exclude_twice_is_unchanged() {
        // given: my vote against a is cast
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        let stored = world.mine(&world.shared, &exclude_key(&a)).await;
        // when: the same action again
        let again = world.apply(MemberAction::Exclude(a, None)).await;
        // then: nothing changed, the record is the same one
        assert_eq!(again, Ok(false));
        assert_eq!(world.mine(&world.shared, &exclude_key(&a)).await, stored, "the vote was rewritten");
    }

    #[tokio::test]
    async fn exclude_confirming_a_proposal_excludes_the_node() {
        // given: four on the list; b has already voted against a
        let world = setup(&[pid(2), pid(3), pid(4)]);
        let (a, b) = (pid(2), pid(3));
        world.given_vote(b, a, 100);
        world.refresh().await;
        assert_eq!(world.verdict(a), Ok(()), "one vote of four is a proposal only");
        // when: I confirm
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: a is excluded by the two votes, in time order
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        assert_eq!(world.ledger().excluded[&a].voters, vec![b, world.me]);
    }

    #[tokio::test]
    async fn exclude_in_a_network_of_two_excludes_at_once() {
        // given: only me and a
        let world = setup(&[pid(2)]);
        let a = pid(2);
        // when: I vote against a
        let changed = world.apply(MemberAction::Exclude(a, None)).await;
        // then: one vote is the threshold for two nodes
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        assert_eq!(world.ledger().excluded[&a].how, How::Votes);
    }

    #[tokio::test]
    async fn exclude_unlisted_is_not_member() {
        // given: a node that is not on the list
        let world = setup(&[pid(2)]);
        let stranger = pid(9);
        let before = world.shared_entries().await;
        // when: I vote to exclude it
        let result = world.apply(MemberAction::Exclude(stranger, None)).await;
        // then: refused, nothing is written
        assert_eq!(result, Err(ActionRefusal::NotMember));
        assert_eq!(world.shared_entries().await, before);
    }
}

mod unexclude {
    use super::*;

    #[tokio::test]
    async fn unexclude_tombstones_vote() {
        // given: my vote against a
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        // when: I take it back
        let changed = world.apply(MemberAction::Unexclude(a)).await;
        // then: the vote is a tombstone and a passes again
        assert_eq!(changed, Ok(true));
        let entry = world.mine(&world.shared, &exclude_key(&a)).await.expect("the key must keep a tombstone");
        assert!(entry.is_tombstone(), "the vote must be withdrawn, not deleted or rewritten");
        assert_eq!(world.verdict(a), Ok(()));
    }

    #[tokio::test]
    async fn unexclude_without_vote_is_unchanged() {
        // given: no vote of mine
        let world = setup(&[pid(2), pid(3)]);
        let before = world.shared_entries().await;
        // when: unexclude
        let changed = world.apply(MemberAction::Unexclude(pid(2))).await;
        // then: nothing changed and nothing is written
        assert_eq!(changed, Ok(false));
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn unexclude_returns_node_held_by_my_vote_alone() {
        // given: me and a only: my single vote has excluded a
        let world = setup(&[pid(2)]);
        let a = pid(2);
        world.apply(MemberAction::Exclude(a, None)).await.unwrap();
        assert_eq!(world.verdict(a), Err(Deny::Excluded));
        // when: I take the vote back
        let changed = world.apply(MemberAction::Unexclude(a)).await;
        // then: a is a member again
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Ok(()));
        assert!(world.ledger().excluded.is_empty(), "nobody is excluded");
    }
}

mod block {
    use super::*;

    #[tokio::test]
    async fn block_touches_own_replica_only() {
        // given: a on the list
        let world = setup(&[pid(2), pid(3)]);
        let a = pid(2);
        world.refresh().await;
        let shared_before = world.shared_entries().await;
        // when: I block a
        let changed = world.apply(MemberAction::Block(a)).await;
        // then: the block is in the own replica, the shared one has not changed, a is refused here
        assert_eq!(changed, Ok(true));
        let entry = world.mine(&world.own, &block_key(&a)).await.expect("the block must be in the own replica");
        assert!(!entry.is_tombstone());
        assert_eq!(world.shared_entries().await, shared_before, "a block must not go to the network");
        assert_eq!(world.verdict(a), Err(Deny::Blocked));
    }

    #[tokio::test]
    async fn block_twice_is_unchanged() {
        // given: a is blocked
        let world = setup(&[pid(2)]);
        world.apply(MemberAction::Block(pid(2))).await.unwrap();
        // when / then: the second block changes nothing
        assert_eq!(world.apply(MemberAction::Block(pid(2))).await, Ok(false));
    }

    #[tokio::test]
    async fn unblock_removes_the_block() {
        // given: a is blocked
        let world = setup(&[pid(2)]);
        let a = pid(2);
        world.apply(MemberAction::Block(a)).await.unwrap();
        // when: unblock
        let changed = world.apply(MemberAction::Unblock(a)).await;
        // then: a passes again, and the own replica holds a tombstone
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(a), Ok(()));
        assert!(world.mine(&world.own, &block_key(&a)).await.is_some_and(|e| e.is_tombstone()));
    }

    #[tokio::test]
    async fn unblock_of_unknown_id_is_unchanged() {
        // given: nothing is blocked
        let world = setup(&[pid(2)]);
        // when / then: unblock of a node I never blocked
        assert_eq!(world.apply(MemberAction::Unblock(pid(9))).await, Ok(false));
    }

    #[tokio::test]
    async fn block_allows_unlisted_node() {
        // given: a node that is not on the list
        let world = setup(&[pid(2)]);
        let stranger = pid(9);
        // when: I block it
        let changed = world.apply(MemberAction::Block(stranger)).await;
        // then: allowed (only a vote needs a member), and the block shows
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(stranger), Err(Deny::Blocked));
    }
}

mod self_target {
    use super::*;

    #[tokio::test]
    async fn self_target_is_refused() {
        // given: a node with a neighbour
        let world = setup(&[pid(2)]);
        let me = world.me;
        let before = (world.shared_entries().await, world.own_entries().await);
        for act in [
            MemberAction::Exclude(me, None),
            MemberAction::Unexclude(me),
            MemberAction::Block(me),
            MemberAction::Unblock(me),
        ] {
            // when: an action names the node itself
            let result = world.apply(act.clone()).await;
            // then: refused (leaving has its own action)
            assert_eq!(result, Err(ActionRefusal::SelfTarget), "{act:?}");
        }
        assert_eq!((world.shared_entries().await, world.own_entries().await), before, "nothing is written");
    }
}

#[path = "actions_leave_tests.rs"]
mod leave_and_failure;

#[path = "actions_reason_tests.rs"]
mod reason_and_withdrawal;

#[path = "actions_admit_tests.rs"]
mod admit_and_unadmit;

// --- setup and helpers ---

struct World {
    access: Access,
    shared: Arc<MemReplica>,
    own: Arc<MemReplica>,
    me: PeerId,
    fail_writes: Arc<AtomicBool>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// The node (seed 1) with `listed` admitted by it; the view starts pending, as in a real start.
fn setup(listed: &[PeerId]) -> World {
    build(listed, false)
}

fn setup_with_failing_shared(listed: &[PeerId]) -> World {
    build(listed, true)
}

fn build(listed: &[PeerId], failing: bool) -> World {
    let net = MemSyncNet::new();
    let node = secret(1);
    let ctx = SyncCtx { secret: node.clone(), network_seed: [9; 32], store_dir: None };
    let backend = net.backend(ctx);
    let (shared, own) = (backend.replica_mem(REPLICA_NET).unwrap(), backend.replica_mem(REPLICA_DOMAINS).unwrap());
    let fail_writes = Arc::new(AtomicBool::new(false));
    let seen_by_access: Arc<dyn Replica> = match failing {
        true => Arc::new(FailingWrites { inner: shared.clone(), fail: fail_writes.clone() }),
        false => shared.clone(),
    };
    let members = Members::with_access(AccessView::pending());
    let access = Access::from_parts(node.peer_id(), seen_by_access, own.clone(), members);
    let world = World { access, shared, own, me: node.peer_id(), fail_writes, _keep: (net, backend) };
    listed.iter().for_each(|p| world.given_admit(world.me, *p));
    world
}

impl World {
    /// An admit record of `author` for `subject` at a time long ago.
    fn given_admit(&self, author: PeerId, subject: PeerId) {
        self.given_record(author, admit_key(&subject), AclKind::Admit, subject, 1);
    }

    /// An exclusion vote of `voter` against `subject` at the given time.
    fn given_vote(&self, voter: PeerId, subject: PeerId, ts_micros: u64) {
        self.given_record(voter, exclude_key(&subject), AclKind::Exclude, subject, ts_micros);
    }

    fn given_record(&self, author: PeerId, key: Vec<u8>, kind: AclKind, subject: PeerId, ts_micros: u64) {
        let value = AclValue { kind, subject, via: Via::Invite, made_unix: 111, note: String::new() };
        let value = Bytes::from(encode(&value));
        self.shared.put_raw(ReplicaEntry { author, key, value, ts_micros });
    }

    async fn apply(&self, act: MemberAction) -> Result<bool, ActionRefusal> {
        apply(&self.access, act).await
    }

    async fn refresh(&self) {
        self.access.refresh().await.expect("the recompute must work on a healthy replica");
    }

    /// The decision of the published view for one node.
    fn verdict(&self, peer: PeerId) -> Result<(), Deny> {
        self.access.view.check(peer)
    }

    fn ledger(&self) -> Arc<Ledger> {
        self.access.view.verdicts().expect("the view must be published after an action").ledger.clone()
    }

    /// My entry under the key, a tombstone included.
    async fn mine(&self, replica: &Arc<MemReplica>, key: &[u8]) -> Option<ReplicaEntry> {
        let all = replica.scan(key).await.unwrap();
        all.into_iter().find(|e| e.author == self.me && e.key == key)
    }

    /// Subjects of my live exclusion votes.
    async fn vote_subjects(&self) -> Vec<PeerId> {
        let all = self.shared.scan(b"acl/exclude/").await.unwrap();
        let mine = all.iter().filter(|e| e.author == self.me && !e.is_tombstone());
        mine.filter_map(|e| match parse_entry(e) {
            Parsed::Record { value, .. } => Some(value.subject),
            _ => None,
        })
        .collect()
    }

    async fn shared_entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }

    async fn own_entries(&self) -> Vec<ReplicaEntry> {
        self.own.scan(b"").await.unwrap()
    }
}

/// A replica that delegates to the in-memory one and can refuse to write.
struct FailingWrites {
    inner: Arc<MemReplica>,
    fail: Arc<AtomicBool>,
}

impl Replica for FailingWrites {
    fn id(&self) -> ReplicaId {
        self.inner.id()
    }

    fn me(&self) -> PeerId {
        self.inner.me()
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        match self.fail.load(Ordering::SeqCst) {
            true => Box::pin(async { Err(NetError::Config("disk is full".into())) }),
            false => self.inner.set_own(key, value),
        }
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        match self.fail.load(Ordering::SeqCst) {
            true => Box::pin(async { Err(NetError::Config("disk is full".into())) }),
            false => self.inner.del_own(key),
        }
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        self.inner.get(author, key)
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.scan(prefix)
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.latest_per_key(prefix)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent> {
        self.inner.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        self.inner.start_sync()
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        self.inner.sync_with(peer)
    }
}
