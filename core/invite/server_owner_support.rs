//! Fixtures of the invite server's owner-path tests: a real `InviteServer` over a filled in-memory domain
//! list holding the issue record of the invite "Masha"; its access view is built by the real tally from a small
//! network (the server is `me` and the root). The owner frame is handed to `owner_reply` directly, without a
//! transport.

use std::sync::{Arc, OnceLock};

use super::*;
use crate::invite::proof::sign_owner;
use crate::invite::store::{issue_record, IssueVerdict};
use crate::invite::wire::{
    encode_owner_request, parse_invite_reply, RedeemReply, OWNER_REQUEST_LEN,
};
use crate::invite::InviteSecret;
use crate::members::access::authority::{Power, PowerIn};
use crate::members::access::record::Via;
use crate::members::access::tally::{Ledger as Votes, TallyInput};
use crate::members::access::view::{AccessView, Verdicts};
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{live, NodeSecret, Replica};

/// The network key with which `setup_node` builds the server.
pub(super) const KEY: [u8; 32] = [1; 32];

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

/// The tally input of a network where the server, its root, admitted every member of `members`.
pub(super) fn input_of(members: &[PeerId]) -> TallyInput {
    let admit = |subject: &PeerId| PowerIn {
        author: me(),
        power: Power::Admit(Via::Invite),
        subject: *subject,
        ts_micros: 1_000_000,
    };
    TallyInput {
        me: me(),
        root: me(),
        votes: Vec::new(),
        powers: members.iter().filter(|m| **m != me()).map(admit).collect(),
    }
}

pub(super) fn access_with(ledger: Votes) -> AccessView {
    let access = AccessView::open();
    access.publish(Verdicts {
        me: me(),
        ledger: Arc::new(ledger),
        blocks: Default::default(),
    });
    access
}

pub(super) fn owner_frame(owner: &NodeSecret) -> [u8; OWNER_REQUEST_LEN] {
    let sig = sign_owner(owner, &issued(), pid(1));
    encode_owner_request(&issued(), owner.peer_id(), &sig)
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
    let key = NetworkKey::from_bytes(KEY);
    let server = InviteServer::new(key.clone(), cell, access, key);
    Node {
        server,
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

    /// Live entries under the prefix in the shared replica.
    pub(super) async fn count(&self, prefix: &str) -> usize {
        live(self.shared.scan(prefix.as_bytes()).await.unwrap()).len()
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
            .verdict(&issued().hash_hex(), now(), None)
            .await;
        assert_eq!(verdict, IssueVerdict::Grant, "the invite is spent");
    }
}
