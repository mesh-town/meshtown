//! Tests of the open-invite list and note edit of `NodeCtl` (`control_invites.rs`): who may ask, and how the
//! outcomes of the edit map to page errors. The ledger sits on an in-memory replica; the viewer's verdicts are made by hand.

use std::collections::BTreeSet;

use bytes::Bytes;

use super::*;
use crate::invite::store::{issue_record, IssueRecord, ISSUE_PREFIX};
use crate::members::access::tally::Ledger as Roles;
use crate::members::access::view::AccessView;
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::ReplicaEntry;

const H1: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[tokio::test]
async fn admin_gets_open_invites() {
    // given: our own invite and another admin's
    let rig = setup(true).await;
    rig.given_issue(rig.me, H1, Some("Anna"));
    rig.given_issue(pid(3), &"ab".repeat(32), None);
    // when
    let reply = rig.when_listed(None).await.unwrap();
    // then
    assert_eq!(reply.invites.len(), 2);
    let own = reply.invites.iter().find(|r| r.hash == H1).unwrap();
    assert!(own.mine);
    assert_eq!(own.for_whom.as_deref(), Some("Anna"));
    assert_eq!(own.issuer, Some(rig.me.to_string()));
    let other = reply.invites.iter().find(|r| r.hash != H1).unwrap();
    assert!(!other.mine);
}

#[tokio::test]
async fn non_admin_is_refused_as_not_admin() {
    // given: the viewer is not an admin
    let rig = setup(false).await;
    rig.given_issue(rig.me, H1, None);
    // when
    let list = rig.when_listed(None).await.unwrap_err();
    let edit = rig.when_listed(Some((H1, Some("x")))).await.unwrap_err();
    // then
    for err in [list, edit] {
        assert!(
            matches!(
                err,
                MemberError::Refused {
                    code: "not_admin",
                    ..
                }
            ),
            "{err:?}"
        );
    }
}

#[tokio::test]
async fn not_issuer_maps_to_refused() {
    // given: the invite was made by another admin
    let rig = setup(true).await;
    rig.given_issue(pid(3), H1, Some("Theirs"));
    // when
    let err = rig.when_listed(Some((H1, Some("Mine")))).await.unwrap_err();
    // then
    assert!(
        matches!(
            err,
            MemberError::Refused {
                code: "not_issuer",
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn not_found_maps_to_action_not_found() {
    // given: no such invite
    let rig = setup(true).await;
    // when
    let err = rig.when_listed(Some((H1, Some("x")))).await.unwrap_err();
    // then
    assert!(
        matches!(&err, MemberError::Action(ActionError::NotFound(m))
            if m == "this invite has been used or has expired"),
        "{err:?}"
    );
}

struct Rig {
    store: crate::invite::store::Ledger,
    replica: std::sync::Arc<crate::transport::net::mem::MemReplica>,
    members: Members,
    me: PeerId,
    admin: bool,
    _keep: (std::sync::Arc<MemSyncNet>, std::sync::Arc<MemSync>),
}

async fn setup(admin: bool) -> Rig {
    let net = MemSyncNet::new();
    let backend = net.backend(SyncCtx {
        secret: secret(2),
        network_seed: [9; 32],
        store_dir: None,
    });
    let shared = backend.replica_mem(REPLICA_NET).unwrap();
    let own = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    let store = crate::invite::store::Ledger::of(&list);
    Rig {
        me: store.me(),
        store,
        replica: shared,
        members: Members::with_access(AccessView::pending()),
        admin,
        _keep: (net, backend),
    }
}

impl Rig {
    /// A v2 issue record of `issuer`, written by the issuer, made a minute ago.
    fn given_issue(&self, issuer: PeerId, hash: &str, who: Option<&str>) {
        let made = unix_now() - 60;
        let rec = IssueRecord {
            v: 2,
            ..issue_record(made, who)
        };
        let mut value = postcard::to_stdvec(&rec).unwrap();
        value.extend(postcard::to_stdvec(&issuer).unwrap());
        self.replica.put_raw(ReplicaEntry {
            author: issuer,
            key: format!("{ISSUE_PREFIX}{hash}").into_bytes(),
            value: Bytes::from(value),
            ts_micros: made * 1_000_000,
        });
    }

    fn verdicts(&self) -> Arc<Verdicts> {
        let mut roles = Roles::empty();
        roles.admins = BTreeSet::from([pid(3)]);
        if self.admin {
            roles.admins.insert(self.me);
        }
        Arc::new(Verdicts {
            me: self.me,
            ledger: Arc::new(roles),
            blocks: BTreeSet::new(),
        })
    }

    async fn when_listed(
        &self,
        edit: Option<(&str, Option<&str>)>,
    ) -> Result<InvitesReply, MemberError> {
        let edit = edit.map(|(h, n)| (h.to_string(), n.map(str::to_string)));
        list_for(&self.store, &self.members, Some(self.verdicts()), edit).await
    }
}
