//! Tests of the member actions in an administered network (`actions.rs`): who may do what, the refusals,
//! and the records the administrator actions write. Each test runs against a real in-memory shared
//! replica with the verdicts recomputed by `Access` under `Rules::Managed`.
//! Nodes: `me` (seed 1) and the root (seed 2, or `me` itself), the member `x` (3), the other administrator
//! `other_admin` (4) and the `outsider` (9), who is never admitted.

// assumed: this file is declared in `actions.rs` as
// `#[cfg(test)] #[path = "actions_managed_tests.rs"] mod managed_tests;`.
// assumed: the new variants are `MemberAction::{Grant(PeerId), Revoke(PeerId), Close}` and
// `ActionRefusal::{NotAdmin, LastAdmin, NotManaged, PeerOnly, Closed}`.

use std::sync::Arc;

use bytes::Bytes;

use crate::members::access::actions::{apply, ActionRefusal, MemberAction};
use crate::members::access::authority::Rules;
use crate::members::access::record::{
    admit_key, encode, exclude_key, grant_key, key_for, revoke_key, AclKind, AclValue, Via,
    CLOSE_KEY,
};
use crate::members::access::runtime::Access;
use crate::members::access::tally::Ledger;
use crate::members::access::view::AccessView;
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{PeerId, Replica, ReplicaEntry};

mod member_without_power {
    use super::*;

    #[tokio::test]
    async fn exclude_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Exclude(member(), None)).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn admit_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Admit(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn grant_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Grant(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn revoke_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Revoke(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn close_is_not_admin() {
        let world = setup(Seat::Member);
        let result = world.apply(MemberAction::Close).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }

    #[tokio::test]
    async fn a_refused_action_writes_nothing() {
        // given: a plain member
        let world = setup(Seat::Member);
        let before = world.shared_entries().await;
        // when: it tries every action of power
        for act in [
            MemberAction::Exclude(member(), None),
            MemberAction::Admit(member()),
            MemberAction::Grant(member()),
            MemberAction::Revoke(member()),
            MemberAction::Close,
        ] {
            let _ = world.apply(act).await;
        }
        // then: the replica is as it was
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn blocking_a_node_stays_open() {
        let world = setup(Seat::Member);
        assert_eq!(world.apply(MemberAction::Block(member())).await, Ok(true));
    }

    #[tokio::test]
    async fn leaving_stays_open() {
        let world = setup(Seat::Member);
        assert_eq!(world.apply(MemberAction::Leave).await, Ok(true));
    }
}

mod forget_votes {
    use super::*;

    #[tokio::test]
    async fn forget_is_peer_only_for_the_administrator() {
        let world = setup(Seat::Root);
        let result = world.apply(MemberAction::Forget(member())).await;
        assert_eq!(result, Err(ActionRefusal::PeerOnly));
    }

    #[tokio::test]
    async fn unforget_is_peer_only_for_the_administrator() {
        let world = setup(Seat::Root);
        let result = world.apply(MemberAction::Unforget(member())).await;
        assert_eq!(result, Err(ActionRefusal::PeerOnly));
    }
}

mod peer_network {
    use super::*;

    #[tokio::test]
    async fn grant_is_not_managed() {
        let world = setup_peer();
        let result = world.apply(MemberAction::Grant(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotManaged));
    }

    #[tokio::test]
    async fn revoke_is_not_managed() {
        let world = setup_peer();
        let result = world.apply(MemberAction::Revoke(member())).await;
        assert_eq!(result, Err(ActionRefusal::NotManaged));
    }

    #[tokio::test]
    async fn close_is_not_managed() {
        let world = setup_peer();
        let result = world.apply(MemberAction::Close).await;
        assert_eq!(result, Err(ActionRefusal::NotManaged));
    }

    #[tokio::test]
    async fn the_peer_actions_work_as_before() {
        let world = setup_peer();
        let result = world.apply(MemberAction::Exclude(member(), None)).await;
        assert_eq!(result, Ok(true));
    }
}

mod last_administrator {
    use super::*;

    #[tokio::test]
    async fn the_only_administrator_may_not_leave() {
        // given: the root alone is the administrator
        let world = setup(Seat::Root);
        // when / then
        let result = world.apply(MemberAction::Leave).await;
        assert_eq!(result, Err(ActionRefusal::LastAdmin));
    }

    #[tokio::test]
    async fn the_only_administrator_may_not_revoke_the_own_role() {
        // given: I am an administrator granted by the root, and the root has left
        let world = setup(Seat::Admin);
        world.given_leave(3, world.root);
        // when / then
        let result = world.apply(MemberAction::Revoke(world.me)).await;
        assert_eq!(result, Err(ActionRefusal::LastAdmin));
    }

    #[tokio::test]
    async fn a_refused_leave_writes_nothing() {
        let world = setup(Seat::Root);
        let before = world.shared_entries().await;
        let _ = world.apply(MemberAction::Leave).await;
        assert_eq!(world.shared_entries().await, before);
    }

    #[tokio::test]
    async fn an_administrator_may_leave_while_another_one_stays() {
        // given: I am an administrator besides the root
        let world = setup(Seat::Admin);
        // when / then
        assert_eq!(world.apply(MemberAction::Leave).await, Ok(true));
    }

    #[tokio::test]
    async fn an_administrator_may_revoke_the_own_role_while_another_one_stays() {
        // given: I am an administrator besides the root
        let world = setup(Seat::Admin);
        // when
        let result = world.apply(MemberAction::Revoke(world.me)).await;
        // then: the role is gone
        assert_eq!(result, Ok(true));
        assert!(!world.ledger().is_admin(&world.me));
    }
}

mod the_root {
    use super::*;

    #[tokio::test]
    async fn the_role_of_the_root_cannot_be_revoked() {
        // given: I am an administrator, the root is another one
        let world = setup(Seat::Admin);
        // when / then
        let result = world.apply(MemberAction::Revoke(world.root)).await;
        assert_eq!(result, Err(ActionRefusal::NotAdmin));
    }
}

mod grant {
    use super::*;

    #[tokio::test]
    async fn a_node_that_is_not_a_member_cannot_be_made_an_administrator() {
        let world = setup(Seat::Root);
        let result = world.apply(MemberAction::Grant(outsider())).await;
        assert_eq!(result, Err(ActionRefusal::NotMember));
    }

    #[tokio::test]
    async fn a_member_becomes_an_administrator() {
        // given: the root and the member x
        let world = setup(Seat::Root);
        // when
        let result = world.apply(MemberAction::Grant(member())).await;
        // then: the record is mine and the member is in the list of administrators
        assert_eq!(result, Ok(true));
        assert!(world.live_ts(&grant_key(&member())).await.is_some());
        assert!(world.ledger().is_admin(&member()));
    }

    #[tokio::test]
    async fn granting_twice_is_unchanged() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        assert_eq!(world.apply(MemberAction::Grant(member())).await, Ok(false));
    }

    #[tokio::test]
    async fn the_role_revoked_can_be_granted_again() {
        // given: x was made an administrator and then lost the role
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        world.apply(MemberAction::Revoke(member())).await.unwrap();
        assert!(!world.ledger().is_admin(&member()));
        // when
        let result = world.apply(MemberAction::Grant(member())).await;
        // then: x is an administrator, and no revoke of mine is newer than the grant
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_admin(&member()));
        let grant = world.live_ts(&grant_key(&member())).await;
        let revoke = world.live_ts(&revoke_key(&member())).await;
        assert!(
            grant.is_some_and(|g| revoke.is_none_or(|r| g > r)),
            "grant {grant:?} must be later than revoke {revoke:?}"
        );
    }

    #[tokio::test]
    async fn revoke_takes_the_role_away() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Grant(member())).await.unwrap();
        let result = world.apply(MemberAction::Revoke(member())).await;
        assert_eq!(result, Ok(true));
        assert!(!world.ledger().is_admin(&member()));
    }
}

mod close {
    use super::*;

    #[tokio::test]
    async fn the_administrator_closes_the_network() {
        // given: the root
        let world = setup(Seat::Root);
        // when
        let result = world.apply(MemberAction::Close).await;
        // then: the record is mine and the verdicts say who closed
        assert_eq!(result, Ok(true));
        assert!(world.live_ts(CLOSE_KEY).await.is_some());
        let closed = world.ledger().closed.clone();
        assert_eq!(closed.map(|c| c.by), Some(world.me));
    }

    #[tokio::test]
    async fn every_action_after_it_is_refused() {
        // given: the network is closed
        let world = setup(Seat::Root);
        world.apply(MemberAction::Close).await.unwrap();
        // assumed: also the actions that stay open in a live network (block, leave) are refused
        for act in [
            MemberAction::Exclude(member(), None),
            MemberAction::Unexclude(member()),
            MemberAction::Admit(member()),
            MemberAction::Grant(member()),
            MemberAction::Revoke(member()),
            MemberAction::Close,
            MemberAction::Block(member()),
            MemberAction::Unblock(member()),
            MemberAction::Leave,
        ] {
            // when / then
            let result = world.apply(act.clone()).await;
            assert_eq!(result, Err(ActionRefusal::Closed), "{act:?}");
        }
    }

    #[tokio::test]
    async fn refused_actions_after_it_write_nothing() {
        let world = setup(Seat::Root);
        world.apply(MemberAction::Close).await.unwrap();
        let before = world.shared_entries().await;
        let _ = world.apply(MemberAction::Exclude(member(), None)).await;
        let _ = world.apply(MemberAction::Grant(member())).await;
        assert_eq!(world.shared_entries().await, before);
    }
}

mod return_of_an_excluded_node {
    use super::*;

    #[tokio::test]
    async fn the_administrator_admits_the_node_back_after_the_own_exclusion() {
        // given: x was excluded by me
        let world = setup(Seat::Root);
        world
            .apply(MemberAction::Exclude(member(), None))
            .await
            .unwrap();
        assert!(!world.ledger().is_member(&member()));
        // when
        let result = world.apply(MemberAction::Admit(member())).await;
        // then: x is a member, and the admission is newer than the exclusion
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_member(&member()));
        let admit = world.live_ts(&admit_key(&member())).await;
        let exclude = world.live_ts(&exclude_key(&member())).await;
        assert!(
            admit.is_some_and(|a| exclude.is_none_or(|e| a > e)),
            "admit {admit:?} must be later than exclude {exclude:?}"
        );
    }

    #[tokio::test]
    async fn the_administrator_admits_the_node_back_after_the_exclusion_by_another_one() {
        // given: x was excluded by the other administrator
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        world.refresh().await;
        assert!(!world.ledger().is_member(&member()));
        // when
        let result = world.apply(MemberAction::Admit(member())).await;
        // then: x is a member again
        assert_eq!(result, Ok(true));
        assert!(world.ledger().is_member(&member()));
    }
}

mod unexclude {
    use super::*;

    #[tokio::test]
    async fn takes_back_only_the_own_exclusion() {
        // given: both the other administrator and I excluded x
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        world.given_exclude(6, world.me, member());
        // when
        let result = world.apply(MemberAction::Unexclude(member())).await;
        // then: my record is gone, the other one stays, and x is still excluded
        assert_eq!(result, Ok(true));
        assert_eq!(world.live_ts(&exclude_key(&member())).await, None);
        assert!(world
            .live_of(other_admin(), &exclude_key(&member()))
            .await
            .is_some());
        assert!(!world.ledger().is_member(&member()));
    }

    #[tokio::test]
    async fn changes_nothing_when_only_another_administrator_excluded() {
        // given: only the other administrator excluded x
        let world = setup(Seat::Root);
        world.given_administrator(other_admin());
        world.given_exclude(5, other_admin(), member());
        // when
        let result = world.apply(MemberAction::Unexclude(member())).await;
        // then: nothing of mine to take back, and the exclusion stands
        assert_eq!(result, Ok(false));
        assert!(world
            .live_of(other_admin(), &exclude_key(&member()))
            .await
            .is_some());
        assert!(!world.ledger().is_member(&member()));
    }
}

// --- setup and helpers ---

/// Where `me` stands in the administered network.
#[derive(Clone, Copy)]
enum Seat {
    /// The creator and the only administrator.
    Root,
    /// Granted the role by the root, who is another administrator.
    Admin,
    /// Admitted by the root, no role.
    Member,
}

/// The member that every scene starts with.
fn member() -> PeerId {
    pid(3)
}

fn other_admin() -> PeerId {
    pid(4)
}

fn outsider() -> PeerId {
    pid(9)
}

struct World {
    access: Access,
    shared: Arc<MemReplica>,
    me: PeerId,
    root: PeerId,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// An administered network with the member `x`; `seat` tells who `me` is.
fn setup(seat: Seat) -> World {
    let me = secret(1).peer_id();
    let root = match seat {
        Seat::Root => me,
        _ => pid(2),
    };
    let world = build(Rules::Managed { root });
    world.given_admit(1, root, member());
    match seat {
        Seat::Root => {}
        Seat::Admin => {
            world.given_admit(1, root, me);
            world.given_grant(2, root, me);
        }
        Seat::Member => world.given_admit(1, root, me),
    }
    world
}

/// A peer network where `me` admitted the member `x`.
fn setup_peer() -> World {
    let world = build(Rules::Peer);
    world.given_admit(1, world.me, member());
    world
}

fn build(rules: Rules) -> World {
    let net = MemSyncNet::new();
    let node = secret(1);
    let ctx = SyncCtx {
        secret: node.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let (shared, own) = (
        backend.replica_mem(REPLICA_NET).unwrap(),
        backend.replica_mem(REPLICA_DOMAINS).unwrap(),
    );
    let members = Members::with_access(AccessView::pending());
    let access = Access::from_parts(node.peer_id(), shared.clone(), own, members).with_rules(rules);
    World {
        access,
        shared,
        me: node.peer_id(),
        root: rules.root().unwrap_or(node.peer_id()),
        _keep: (net, backend),
    }
}

impl World {
    /// An admission written by `author` at second `at`.
    fn given_admit(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Admit, subject);
    }

    fn given_grant(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Grant, subject);
    }

    /// An exclusion vote of `author` against `subject` at second `at`.
    fn given_exclude(&self, at: u64, author: PeerId, subject: PeerId) {
        self.given_record(at, author, AclKind::Exclude, subject);
    }

    /// `who` leaves: an exclusion vote against itself.
    fn given_leave(&self, at: u64, who: PeerId) {
        self.given_exclude(at, who, who);
    }

    /// `node` is admitted and made an administrator by the root.
    fn given_administrator(&self, node: PeerId) {
        self.given_admit(1, self.root, node);
        self.given_grant(2, self.root, node);
    }

    fn given_record(&self, at: u64, author: PeerId, kind: AclKind, subject: PeerId) {
        let value = AclValue {
            kind,
            subject,
            via: Via::Invite,
            made_unix: at,
            note: String::new(),
        };
        self.shared.put_raw(ReplicaEntry {
            author,
            key: key_for(kind, &subject),
            value: Bytes::from(encode(&value)),
            ts_micros: at * 1_000_000,
        });
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

    fn ledger(&self) -> Arc<Ledger> {
        self.access
            .view
            .verdicts()
            .expect("the view must be published after an action")
            .ledger
            .clone()
    }

    /// The time of my live record under the key.
    async fn live_ts(&self, key: &[u8]) -> Option<u64> {
        self.live_of(self.me, key).await.map(|e| e.ts_micros)
    }

    /// The live record of `author` under exactly this key.
    async fn live_of(&self, author: PeerId, key: &[u8]) -> Option<ReplicaEntry> {
        let all = self.shared.scan(key).await.unwrap();
        all.into_iter()
            .find(|e| e.author == author && e.key == key && !e.is_tombstone())
    }

    async fn shared_entries(&self) -> Vec<ReplicaEntry> {
        self.shared.scan(b"").await.unwrap()
    }
}
