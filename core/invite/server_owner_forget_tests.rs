//! Tests of the invite server's owner path for owners that the membership list removed: a forgotten
//! owner and an owner the server itself voted to remove come back by a valid invite, and the admit
//! record the server writes is stamped later than every forget vote it sees, so the return outdates them.
//!
//! The server is a real `InviteServer` over a filled in-memory domain list holding the issue record of the
//! invite "Masha"; its access view is built by the real tally from a small network (the server is `me`).
//! The owner frame is handed to `owner_reply` directly, without a transport.

use std::sync::{Arc, OnceLock};

use super::*;
use crate::invite::proof::sign_owner;
use crate::invite::store::{issue_record, IssueVerdict};
use crate::invite::wire::{
    encode_owner_request, parse_invite_reply, RedeemReply, OWNER_REQUEST_LEN,
};
use crate::invite::InviteSecret;
use crate::members::access::record::{
    admit_key, parse_entry, put, AclKind, AclValue, Parsed, Via,
};
use crate::members::access::tally::{
    tally, AdmitIn, Exclusion, ForgetIn, ForgetRound, How, Ledger as Votes, TallyInput,
};
use crate::members::access::view::{AccessView, Verdicts};
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{live, NodeSecret, Replica};

/// The network key with which `setup_node` builds the server.
const KEY: [u8; 32] = [1; 32];

mod owner_code_of_the_verdict {
    use super::*;

    #[test]
    fn return_cases_go_on() {
        // given / when / then: no refusal for a member, an unknown key, a forgotten one, one I voted out
        for case in [
            Ok(()),
            Err(Deny::Unlisted),
            Err(Deny::Forgotten),
            Err(Deny::RemovedByMe),
        ] {
            assert_eq!(owner_code(case), None, "{case:?} must let the owner go on");
        }
    }

    #[test]
    fn excluded_stays_final() {
        assert_eq!(
            owner_code(Err(Deny::Excluded)),
            Some(RedeemCode::Excluded)
        );
    }

    #[test]
    fn own_exclusion_vote_block_and_leaving_stay_retryable() {
        for case in [Deny::VotedByMe, Deny::Blocked, Deny::Left, Deny::Refused] {
            assert_eq!(
                owner_code(Err(case)),
                Some(RedeemCode::NotReady),
                "{case:?} must be a retryable refusal"
            );
        }
    }
}

mod forgotten_owner {
    use super::*;

    #[tokio::test]
    async fn gets_key_and_admit_and_the_invite_is_burned() {
        // given: the server forgot the owner (two members, one vote forgets) long ago
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), 100)])).await;
        node.then_view_says(owner_id(), Err(Deny::Forgotten));
        // when: the owner presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: the key, the owner's admit record and the used mark
        assert_key(reply);
        assert!(node.admit_of(owner_id()).await.is_some(), "no admit record");
        assert_eq!(
            node.count("invite-used/").await,
            1,
            "the invite was not burned"
        );
    }

    #[tokio::test]
    async fn admit_is_stamped_later_than_the_forget_vote_in_the_future() {
        // given: the forget vote carries a time far ahead of the server's clock
        let mark = now() + 10_000;
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), mark)])).await;
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the admit is one second after the vote, not at the clock
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert_eq!(admit.made_unix, mark + 1, "the admit is not later than the vote");
    }

    #[tokio::test]
    async fn admit_is_stamped_with_the_clock_when_the_forget_vote_is_old() {
        // given: an old forget vote
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), 100)])).await;
        let before = now();
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the stamp is the clock reading of the grant
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix >= before && admit.made_unix <= now(),
            "made_unix {} is off the clock",
            admit.made_unix
        );
    }

    #[tokio::test]
    async fn own_older_admit_is_overwritten_with_a_later_one() {
        // given: the server's old admit record of the owner is in the list, then the owner was forgotten
        let mark = now() + 500;
        let node = setup_node(view_of(&[owner_id()], vec![forget(owner_id(), mark)])).await;
        node.given_own_admit(owner_id(), 5).await;
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then: the record under the same key is the new one
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix > mark,
            "the old admit (5) was not overwritten: {}",
            admit.made_unix
        );
        assert_eq!(node.count("acl/admit/").await, 1, "a second record appeared");
    }

    #[tokio::test]
    async fn without_a_forget_vote_the_admit_is_stamped_with_the_clock() {
        // given: an owner nobody voted about (unknown to the list)
        let node = setup_node(view_of(&[], Vec::new())).await;
        let before = now();
        // when
        assert_key(node.when_owner_asks(&secret(7)).await);
        // then
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        assert!(
            admit.made_unix >= before && admit.made_unix <= now(),
            "made_unix {} is off the clock",
            admit.made_unix
        );
    }
}

mod owner_removed_by_my_vote {
    use super::*;

    #[tokio::test]
    async fn waiting_round_owner_invite_spends_round() {
        // given: four members, so one vote is not enough; my forget vote about the owner waits for a second
        let members = [pid(3), pid(4), owner_id()];
        let input = input_of(&members, vec![forget(owner_id(), 100)]);
        let node = setup_node(access_of(&input)).await;
        node.then_view_says(owner_id(), Err(Deny::RemovedByMe));
        // when: the owner presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: the key and an admit later than my vote; counted again, the round is gone and the owner is a member
        assert_key(reply);
        let admit = node.admit_of(owner_id()).await.expect("no admit record");
        let after = verdicts_of(&with_admit(input, owner_id(), admit.made_unix));
        assert!(
            after.ledger.forgets.is_empty(),
            "the round is still open: {:?}",
            after.ledger.forgets
        );
        assert!(after.my_forgets().is_empty(), "my vote still stands");
        assert!(after.ledger.is_member(&owner_id()));
    }

    #[tokio::test]
    async fn others_waiting_round_does_not_stop_the_invite() {
        // given: someone else's vote about the owner waits, mine does not exist
        let members = [pid(3), pid(4), owner_id()];
        let input = input_of(&members, vec![forget_by(pid(3), owner_id(), 100)]);
        let node = setup_node(access_of(&input)).await;
        node.then_view_says(owner_id(), Ok(()));
        // when / then
        assert_key(node.when_owner_asks(&secret(7)).await);
    }
}

mod excluded_and_forgotten_owner {
    use super::*;

    #[tokio::test]
    async fn gets_excluded_and_invite_unspent() {
        // given: the ledger holds the owner as excluded and as forgotten at once
        let node = setup_node(access_with(excluded_and_forgotten(owner_id()))).await;
        node.then_view_says(owner_id(), Err(Deny::Excluded));
        // when
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: Excluded wins, nothing is written, the invite works for another key
        assert_refused(reply, RedeemCode::Excluded);
        node.then_invite_unspent().await;
    }
}

/// The server's own id: the replica author, the inbox transport `secret(2)`.
fn me() -> PeerId {
    pid(2)
}

fn owner_id() -> PeerId {
    pid(7)
}

fn issued() -> InviteSecret {
    InviteSecret::from_bytes([7; 32])
}

fn now() -> u64 {
    crate::transport::net_mainline::unix_now()
}

/// A forget vote of the server about `subject` made at `made_unix`.
fn forget(subject: PeerId, made_unix: u64) -> ForgetIn {
    forget_by(me(), subject, made_unix)
}

fn forget_by(author: PeerId, subject: PeerId, made_unix: u64) -> ForgetIn {
    ForgetIn {
        author,
        subject,
        made_unix,
    }
}

/// The tally input of a network where the server admitted every member of `members` at second 0.
fn input_of(members: &[PeerId], forgets: Vec<ForgetIn>) -> TallyInput {
    let admit = |subject: &PeerId| AdmitIn {
        author: me(),
        subject: *subject,
        via: Via::Invite,
        made_unix: 0,
    };
    TallyInput {
        me: me(),
        admits: members.iter().map(admit).collect(),
        votes: Vec::new(),
        forgets,
    }
}

/// The same input where the server admits `subject` again at `made_unix`.
fn with_admit(mut input: TallyInput, subject: PeerId, made_unix: u64) -> TallyInput {
    input.admits.retain(|a| a.subject != subject);
    input.admits.push(AdmitIn {
        author: me(),
        subject,
        via: Via::Invite,
        made_unix,
    });
    input
}

fn verdicts_of(input: &TallyInput) -> Verdicts {
    Verdicts {
        me: me(),
        ledger: Arc::new(tally(input)),
        my_votes: Default::default(),
        blocks: Default::default(),
        reasons: Default::default(),
    }
}

fn access_of(input: &TallyInput) -> AccessView {
    access_from(verdicts_of(input))
}

fn access_from(verdicts: Verdicts) -> AccessView {
    let access = AccessView::open();
    access.publish(verdicts);
    access
}

/// A two-node network (the server and the members) with the given forget votes.
fn view_of(members: &[PeerId], forgets: Vec<ForgetIn>) -> AccessView {
    access_of(&input_of(members, forgets))
}

/// A ledger where the node is both excluded by votes and forgotten, with forget marks.
fn excluded_and_forgotten(owner: PeerId) -> Votes {
    let mut ledger = tally(&input_of(&[owner], Vec::new()));
    ledger.excluded.insert(
        owner,
        Exclusion {
            how: How::Votes,
            at_micros: 10,
            voters: Vec::new(),
        },
    );
    let round = ForgetRound {
        voters: vec![me()],
        at_unix: Some(100),
    };
    ledger.forgets.insert(owner, round);
    ledger.forget_marks.insert(owner, 100);
    ledger
}

fn access_with(ledger: Votes) -> AccessView {
    access_from(Verdicts {
        me: me(),
        ledger: Arc::new(ledger),
        my_votes: Default::default(),
        blocks: Default::default(),
        reasons: Default::default(),
    })
}

fn owner_frame(owner: &NodeSecret) -> [u8; OWNER_REQUEST_LEN] {
    let sig = sign_owner(owner, &issued(), pid(1));
    encode_owner_request(&issued(), owner.peer_id(), &sig)
}

fn assert_key(reply: Option<RedeemReply>) {
    match reply {
        Some(RedeemReply::Key(key)) => {
            assert_eq!(key.to_bytes(), KEY, "the key of another network")
        }
        other => panic!("expected the network key, got {other:?}"),
    }
}

fn assert_refused(reply: Option<RedeemReply>, want: RedeemCode) {
    match reply {
        Some(RedeemReply::Refused(got)) => {
            assert_eq!(got, want, "the refusal code is not the expected one")
        }
        other => panic!("expected the refusal {want:?}, got {other:?}"),
    }
}

/// The server over a filled list with one issued invite.
struct Node {
    server: InviteServer,
    access: AccessView,
    shared: Arc<dyn Replica>,
    list: DomainList,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

async fn setup_node(access: AccessView) -> Node {
    let net = MemSyncNet::new();
    let ctx = SyncCtx {
        secret: secret(2),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let shared: Arc<dyn Replica> = backend.replica_mem(REPLICA_NET).unwrap();
    let own: Arc<dyn Replica> = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    let record = issue_record(now(), Some("Masha"));
    Ledger::of(&list)
        .issue(&issued().hash_hex(), &record)
        .await
        .unwrap();
    let cell = Arc::new(OnceLock::new());
    drop(cell.set(list.clone()));
    let server = InviteServer::new(NetworkKey::from_bytes(KEY), cell, access.clone());
    Node {
        server,
        access,
        shared,
        list,
        _keep: (net, backend),
    }
}

impl Node {
    /// The owner presents the invite over a connection whose transport id is `pid(1)`.
    async fn when_owner_asks(&self, owner: &NodeSecret) -> Option<RedeemReply> {
        let reply = self
            .server
            .owner_reply(&owner_frame(owner), pid(1), "test")
            .await;
        parse_invite_reply(&reply)
    }

    /// The precondition: the server's view gives this verdict about the node.
    fn then_view_says(&self, peer: PeerId, want: Result<(), Deny>) {
        assert_eq!(self.access.check(peer), want, "the view is not as the test needs");
    }

    /// Live entries under the prefix in the shared replica.
    async fn count(&self, prefix: &str) -> usize {
        live(self.shared.scan(prefix.as_bytes()).await.unwrap()).len()
    }

    /// The server's own admit record of `subject` by invite, made at `made_unix`.
    async fn given_own_admit(&self, subject: PeerId, made_unix: u64) {
        let value = AclValue {
            kind: AclKind::Admit,
            subject,
            via: Via::Invite,
            made_unix,
            note: String::new(),
        };
        put(&*self.shared, &value).await.unwrap();
    }

    /// The server's own parsed admit record of `subject`.
    async fn admit_of(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self.shared.get(me(), &admit_key(&subject)).await.unwrap()?;
        match parse_entry(&entry) {
            Parsed::Record { value, .. } => Some(value),
            other => panic!("not a record under the admit key: {other:?}"),
        }
    }

    /// No claim, no used mark, no admit, and the storage still grants the invite.
    async fn then_invite_unspent(&self) {
        for prefix in ["invite-claim/", "invite-used/", "acl/admit/"] {
            assert_eq!(
                self.count(prefix).await,
                0,
                "something was written under {prefix}"
            );
        }
        let verdict = Ledger::of(&self.list)
            .verdict(&issued().hash_hex(), now())
            .await;
        assert_eq!(verdict, IssueVerdict::Grant, "the invite is spent");
    }
}
