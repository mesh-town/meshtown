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
use crate::members::access::record::{admit_key, parse_entry, put, AclKind, AclValue, Parsed, Via};
use crate::members::access::tally::{
    tally, AdmitIn, Exclusion, ForgetIn, ForgetRound, How, Ledger as Votes, Rules, TallyInput,
};
use crate::members::access::view::{AccessView, Verdicts};
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{live, NodeSecret, Replica};

/// The network key with which `setup_node` builds the server.
pub(super) const KEY: [u8; 32] = [1; 32];

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
        assert_eq!(owner_code(Err(Deny::Excluded)), Some(RedeemCode::Excluded));
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

/// The server's own id: the replica author, the inbox transport `secret(2)`.
pub(super) fn me() -> PeerId {
    pid(2)
}

pub(super) fn owner_id() -> PeerId {
    pid(7)
}

pub(super) fn issued() -> InviteSecret {
    InviteSecret::from_bytes([7; 32])
}

pub(super) fn now() -> u64 {
    crate::transport::net_mainline::unix_now()
}

/// A forget vote of the server about `subject` made at `made_unix`.
pub(super) fn forget(subject: PeerId, made_unix: u64) -> ForgetIn {
    forget_by(me(), subject, made_unix)
}

pub(super) fn forget_by(author: PeerId, subject: PeerId, made_unix: u64) -> ForgetIn {
    ForgetIn {
        author,
        subject,
        made_unix,
    }
}

/// The tally input of a network where the server admitted every member of `members` at second 0.
pub(super) fn input_of(members: &[PeerId], forgets: Vec<ForgetIn>) -> TallyInput {
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
        rules: Rules::Peer,
        powers: vec![],
    }
}

/// The same input where the server admits `subject` again at `made_unix`.
pub(super) fn with_admit(mut input: TallyInput, subject: PeerId, made_unix: u64) -> TallyInput {
    input.admits.retain(|a| a.subject != subject);
    input.admits.push(AdmitIn {
        author: me(),
        subject,
        via: Via::Invite,
        made_unix,
    });
    input
}

pub(super) fn verdicts_of(input: &TallyInput) -> Verdicts {
    Verdicts {
        me: me(),
        ledger: Arc::new(tally(input)),
        my_votes: Default::default(),
        blocks: Default::default(),
        reasons: Default::default(),
    }
}

pub(super) fn access_of(input: &TallyInput) -> AccessView {
    access_from(verdicts_of(input))
}

pub(super) fn access_from(verdicts: Verdicts) -> AccessView {
    let access = AccessView::open();
    access.publish(verdicts);
    access
}

/// A two-node network (the server and the members) with the given forget votes.
pub(super) fn view_of(members: &[PeerId], forgets: Vec<ForgetIn>) -> AccessView {
    access_of(&input_of(members, forgets))
}

/// A ledger where the node is both excluded by votes and forgotten, with forget marks.
pub(super) fn excluded_and_forgotten(owner: PeerId) -> Votes {
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

pub(super) fn access_with(ledger: Votes) -> AccessView {
    access_from(Verdicts {
        me: me(),
        ledger: Arc::new(ledger),
        my_votes: Default::default(),
        blocks: Default::default(),
        reasons: Default::default(),
    })
}

pub(super) fn owner_frame(owner: &NodeSecret) -> [u8; OWNER_REQUEST_LEN] {
    let sig = sign_owner(owner, &issued(), pid(1));
    encode_owner_request(&issued(), owner.peer_id(), &sig)
}

pub(super) fn assert_key(reply: Option<RedeemReply>) {
    match reply {
        Some(RedeemReply::Key(key)) => {
            assert_eq!(key.to_bytes(), KEY, "the key of another network")
        }
        other => panic!("expected the network key, got {other:?}"),
    }
}

pub(super) fn assert_refused(reply: Option<RedeemReply>, want: RedeemCode) {
    match reply {
        Some(RedeemReply::Refused(got)) => {
            assert_eq!(got, want, "the refusal code is not the expected one")
        }
        other => panic!("expected the refusal {want:?}, got {other:?}"),
    }
}

/// The server over a filled list with one issued invite.
pub(super) struct Node {
    server: InviteServer,
    access: AccessView,
    shared: Arc<dyn Replica>,
    list: DomainList,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

pub(super) async fn setup_node(access: AccessView) -> Node {
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
    pub(super) async fn when_owner_asks(&self, owner: &NodeSecret) -> Option<RedeemReply> {
        let reply = self
            .server
            .owner_reply(&owner_frame(owner), pid(1), "test")
            .await;
        parse_invite_reply(&reply)
    }

    /// The precondition: the server's view gives this verdict about the node.
    pub(super) fn then_view_says(&self, peer: PeerId, want: Result<(), Deny>) {
        assert_eq!(
            self.access.check(peer),
            want,
            "the view is not as the test needs"
        );
    }

    /// Live entries under the prefix in the shared replica.
    pub(super) async fn count(&self, prefix: &str) -> usize {
        live(self.shared.scan(prefix.as_bytes()).await.unwrap()).len()
    }

    /// The server's own admit record of `subject` by invite, made at `made_unix`.
    pub(super) async fn given_own_admit(&self, subject: PeerId, made_unix: u64) {
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
    pub(super) async fn admit_of(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self.shared.get(me(), &admit_key(&subject)).await.unwrap()?;
        match parse_entry(&entry) {
            Parsed::Record { value, .. } => Some(value),
            other => panic!("not a record under the admit key: {other:?}"),
        }
    }

    /// No claim, no used mark, no admit, and the storage still grants the invite.
    pub(super) async fn then_invite_unspent(&self) {
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
