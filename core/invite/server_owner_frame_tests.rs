//! Tests of the invite server on the owner frame (kind 3) and of its refusal of the legacy frame (kind 1).
//!
//! The server is a real `InviteServer` over a filled in-memory domain list that holds the issue record
//! of the invite "Masha"; the access view is set per test. What the server must do: prove the owner
//! first, ask the access view, and only then claim the invite; the admit record is written before the key.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::*;
use crate::invite::store::{issue_record, IssueVerdict, Ledger};
use crate::members::access::record::{admit_key, parse_entry, AclKind, AclValue, Parsed, Via};
use crate::members::access::tally::{
    tally, AdmitIn, Exclusion, How, Ledger as Votes, Rules, TallyInput, VoteIn,
};
use crate::members::access::view::Verdicts;
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{live, Replica};

mod legacy_frame {
    use super::*;

    #[tokio::test]
    async fn kind_1_gets_bad_request_and_no_claim() {
        // given: an issued invite and an open access view
        let node = setup_node(AccessView::open()).await;
        let session = node.fx.when_client_connects().await;
        // when: an old client sends the 33-byte kind 1 frame with the right secret
        let reply = session.when_asks(&valid_frame()).await;
        // then: BadRequest, and the invite is untouched: no claim, no used mark, no admit
        assert_refused(reply, RedeemCode::BadRequest);
        node.then_invite_unspent().await;
    }
}

mod owner_frame {
    use super::*;

    #[tokio::test]
    async fn owner_frame_admits_owner_then_burns() {
        // given: an issued invite "Masha"
        let node = setup_node(AccessView::open()).await;
        // when: the owner presents it
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: the key, the server's own admit of the owner by invite with the note, and the used mark
        assert_key(reply);
        let want = AclValue {
            kind: AclKind::Admit,
            subject: pid(7),
            via: Via::Invite,
            made_unix: 0,
            note: "Masha".into(),
        };
        let got = node
            .admit_of(pid(7))
            .await
            .expect("the owner was not admitted");
        assert_eq!(
            (got.kind, got.subject, got.via, got.note),
            (want.kind, want.subject, want.via, want.note)
        );
        assert_eq!(
            node.count("invite-used/").await,
            1,
            "the invite was not burned"
        );
    }

    #[tokio::test]
    async fn bad_proof_gets_bad_request_and_no_claim() {
        // given: a frame whose signature is damaged
        let node = setup_node(AccessView::open()).await;
        let mut frame = owner_frame(&secret(7), &issued(), pid(1));
        frame[100] ^= 1;
        let session = node.fx.when_client_connects().await;
        // when
        let reply = session.when_asks(&frame).await;
        // then
        assert_refused(reply, RedeemCode::BadRequest);
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn proof_for_other_transport_gets_bad_request() {
        // given: a proof made for another transport id than the one that connects
        let node = setup_node(AccessView::open()).await;
        let session = node.fx.when_client_connects().await;
        // when: the client replays a frame signed for pid(9)
        let reply = session
            .when_asks(&owner_frame(&secret(7), &issued(), pid(9)))
            .await;
        // then
        assert_refused(reply, RedeemCode::BadRequest);
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn excluded_owner_gets_excluded_and_invite_unspent() {
        // given: a network of two where the server voted its newcomer out
        let node = setup_node(excluded_view(server_id(), pid(7))).await;
        // when: the excluded owner presents the invite
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: Excluded, and the invite is still unspent
        assert_refused(reply, RedeemCode::Excluded);
        node.then_invite_unspent().await;
        // when: the same secret comes with another key
        let again = node.when_owner_asks(&secret(8)).await;
        // then: the invite works for the new key
        assert_key(again);
    }

    #[tokio::test]
    async fn blocked_owner_gets_not_ready() {
        // given: the server blocked this owner locally
        let node = setup_node(view(&[server_id(), pid(7)], &[], &[], &[pid(7)])).await;
        // when / then: a retryable refusal: another holder may decide otherwise
        assert_refused(node.when_owner_asks(&secret(7)).await, RedeemCode::NotReady);
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn owner_voted_by_me_gets_not_ready() {
        // given: the server voted against this owner, the vote is not enough to exclude
        let node = setup_node(view(&[server_id(), pid(7)], &[], &[pid(7)], &[])).await;
        // when / then
        assert_refused(node.when_owner_asks(&secret(7)).await, RedeemCode::NotReady);
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn server_that_left_gets_not_ready() {
        // given: the server itself is excluded (it left the network)
        let node = setup_node(view(&[server_id()], &[server_id()], &[], &[])).await;
        // when / then: it does not hand out keys, but another holder may
        assert_refused(node.when_owner_asks(&secret(7)).await, RedeemCode::NotReady);
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn pending_view_gets_not_ready_within_read_wait() {
        // given: the access view has not published yet
        let node = setup_node(AccessView::pending()).await;
        let started = Instant::now();
        // when
        let reply = node.when_owner_asks(&secret(7)).await;
        // then: NotReady after a short wait, not after the answer timeout
        assert_refused(reply, RedeemCode::NotReady);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "waited {:?} for the view",
            started.elapsed()
        );
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn short_owner_frame_ends_the_handler() {
        // given: 100 of 129 bytes and the end of the stream
        let node = setup_node(AccessView::open()).await;
        let mut session = node.fx.when_client_connects().await;
        let frame = owner_frame(&secret(7), &issued(), pid(1));
        // when
        let reply = session.when_asks(&frame[..100]).await;
        // then: no reply, the handler finished, the invite is untouched
        assert!(
            reply.is_none(),
            "a reply came to a cut-off owner frame: {reply:?}"
        );
        session.then_handler_ends().await;
        node.then_invite_unspent().await;
    }

    #[tokio::test]
    async fn repeat_after_ok_is_used() {
        // given: the invite is redeemed once
        let node = setup_node(AccessView::open()).await;
        assert_key(node.when_owner_asks(&secret(7)).await);
        // when: the same owner presents it again
        let again = node.when_owner_asks(&secret(7)).await;
        // then
        assert_refused(again, RedeemCode::Used);
    }
}

/// The invite secret that `setup_node` issued (the one `valid_frame` carries).
fn issued() -> InviteSecret {
    InviteSecret::from_bytes([7; 32])
}

/// The id of the server's node: the inbox transport `secret(2)` is also the author of its replica.
fn server_id() -> PeerId {
    pid(2)
}

/// A ledger where `admitted` are admitted and `excluded` are excluded by votes.
fn ledger_of(admitted: &[PeerId], excluded: &[PeerId]) -> Votes {
    let by_votes = || Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    Votes {
        admitted: admitted.iter().copied().collect(),
        excluded: excluded.iter().map(|p| (*p, by_votes())).collect(),
        pending: Default::default(),
        need: 1,
        admissions: Default::default(),
        forgets: Default::default(),
        admit_unix: Default::default(),
        forget_marks: Default::default(),
        forget_need: 1,
        admins: Default::default(),
        closed: None,
    }
}

/// An open view that has published the server's verdicts over the given facts.
fn view(
    admitted: &[PeerId],
    excluded: &[PeerId],
    my_votes: &[PeerId],
    blocks: &[PeerId],
) -> AccessView {
    let verdicts = Verdicts {
        me: server_id(),
        ledger: Arc::new(ledger_of(admitted, excluded)),
        my_votes: my_votes.iter().copied().collect(),
        blocks: blocks.iter().copied().collect::<BTreeSet<_>>(),
        reasons: Default::default(),
    };
    let access = AccessView::open();
    access.publish(verdicts);
    access
}

/// The view of a network of two, `me` and `owner`, where `me` voted `owner` out (the tally decides).
fn excluded_view(me: PeerId, owner: PeerId) -> AccessView {
    let input = TallyInput {
        me,
        admits: vec![AdmitIn {
            author: me,
            subject: owner,
            via: Via::Invite,
            made_unix: 0,
        }],
        votes: vec![VoteIn {
            voter: me,
            subject: owner,
            ts_micros: 10,
        }],
        forgets: Vec::new(),
        rules: Rules::Peer,
        powers: vec![],
    };
    let verdicts = Verdicts {
        me,
        ledger: Arc::new(tally(&input)),
        my_votes: BTreeSet::from([owner]),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    };
    let access = AccessView::open();
    access.publish(verdicts);
    access
}

fn assert_key(reply: Option<RedeemReply>) {
    match reply {
        Some(RedeemReply::Key(key)) => {
            assert_eq!(key.to_bytes(), KEY, "the key of another network")
        }
        other => panic!("expected the network key, got {other:?}"),
    }
}

/// The server node over a filled list with one issued invite.
struct Node {
    fx: Fixture,
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
    let record = issue_record(crate::transport::net_mainline::unix_now(), Some("Masha"));
    Ledger::of(&list)
        .issue(&issued().hash_hex(), &record)
        .await
        .unwrap();
    let fx = setup(Opts {
        access,
        list: Some(list.clone()),
        ..Opts::default()
    });
    Node {
        fx,
        shared,
        list,
        _keep: (net, backend),
    }
}

impl Node {
    /// A new connection from the client transport `pid(1)` carrying the owner frame of `owner`.
    async fn when_owner_asks(&self, owner: &NodeSecret) -> Option<RedeemReply> {
        let session = self.fx.when_client_connects().await;
        session
            .when_asks(&owner_frame(owner, &issued(), pid(1)))
            .await
    }

    /// Live entries under the prefix in the shared replica.
    async fn count(&self, prefix: &str) -> usize {
        live(self.shared.scan(prefix.as_bytes()).await.unwrap()).len()
    }

    /// The server's own parsed admit record of `subject`.
    async fn admit_of(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self
            .shared
            .get(server_id(), &admit_key(&subject))
            .await
            .unwrap()?;
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
            .verdict(
                &issued().hash_hex(),
                crate::transport::net_mainline::unix_now(),
            )
            .await;
        assert_eq!(verdict, IssueVerdict::Grant, "the invite is spent");
    }
}
