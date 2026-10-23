//! Tests of the actions of a node that the others forgot (`actions.rs`): its votes and admissions would be
//! dropped by every tally, so they are refused; leaving and the takebacks stay open.

use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, key_for};
use crate::members::access::view::AccessView;
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::ReplicaEntry;

/// The node `me` (seed 1) that admitted `a` (2) and `b` (3), after both of them forgot it.
struct Forgotten {
    access: Access,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>, Arc<MemReplica>),
}

fn forgotten_me() -> Forgotten {
    let net = MemSyncNet::new();
    let secret = secret(1);
    let ctx = SyncCtx {
        secret: secret.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let shared = backend.replica_mem(REPLICA_NET).unwrap();
    let own = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let members = Members::with_access(AccessView::pending());
    let me = secret.peer_id();
    let access = Access::from_parts(me, shared.clone(), own, members);
    let (a, b) = (pid(2), pid(3));
    put(&shared, me, AclKind::Admit, a, 111);
    put(&shared, me, AclKind::Admit, b, 111);
    put(&shared, a, AclKind::Forget, me, 200);
    put(&shared, b, AclKind::Forget, me, 200);
    Forgotten {
        access,
        _keep: (net, backend, shared),
    }
}

fn put(shared: &MemReplica, author: PeerId, kind: AclKind, subject: PeerId, made_unix: u64) {
    let value = AclValue {
        kind,
        subject,
        via: Via::Invite,
        made_unix,
        note: String::new(),
    };
    shared.put_raw(ReplicaEntry {
        author,
        key: key_for(kind, &subject),
        value: Bytes::from(encode(&value)),
        ts_micros: 1,
    });
}

mod forgotten_node {
    use super::*;

    #[tokio::test]
    async fn cannot_forget_exclude_or_admit() {
        // given: both other members forgot me
        let node = forgotten_me();
        // when / then: records of mine would be dead, so the actions are refused
        for act in [
            MemberAction::Forget(pid(2)),
            MemberAction::Exclude(pid(2), None),
            MemberAction::Admit(pid(9)),
            MemberAction::Block(pid(2)),
        ] {
            assert_eq!(
                apply(&node.access, act.clone()).await,
                Err(ActionRefusal::Left),
                "{act:?} must be refused"
            );
        }
    }

    #[tokio::test]
    async fn may_still_leave() {
        // given / when
        let node = forgotten_me();
        // then
        assert_eq!(apply(&node.access, MemberAction::Leave).await, Ok(true));
    }

    #[tokio::test]
    async fn takebacks_are_not_refused_as_removed() {
        // given / when / then: the refusal, if any, is about the missing record and not about being removed
        let node = forgotten_me();
        assert_eq!(
            apply(&node.access, MemberAction::Unforget(pid(2))).await,
            Err(ActionRefusal::NotForgotten)
        );
        assert!(apply(&node.access, MemberAction::Unblock(pid(2)))
            .await
            .is_ok());
    }
}
