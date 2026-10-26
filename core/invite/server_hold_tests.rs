//! Tests of the invite server on the HOLD request (kind 2): who may ask a holder to keep a copy of an issue
//! record and whose name the copy carries.
//!
//! The server is a real `InviteServer` over a filled in-memory domain list; the client is the transport
//! `pid(1)`, the server (the holder) is `pid(2)`. Only an admin's HOLD is kept, and the copy names the sender
//! as the issuer.

use std::collections::BTreeSet;

use bytes::Bytes;

use super::*;
use crate::invite::head::hash_hex_of;
use crate::invite::store::{issue_record, IssueVerdict, Ledger, ISSUE_PREFIX};
use crate::invite::store_admins::issue_value;
use crate::members::access::tally::Ledger as Votes;
use crate::members::access::view::Verdicts;
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{Replica, ReplicaEntry};
use crate::transport::net_mainline::unix_now;

mod holder {
    use super::*;

    mod hold_from_a_non_admin {
        use super::*;

        #[tokio::test]
        async fn a_non_admin_sender_is_refused() {
            // given: the holder is an admin, the sender is not
            let holder = setup_holder(admins_view(&[pid(2)])).await;
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then
            assert_eq!(code, Some(RedeemCode::NotFromAdmin));
        }

        #[tokio::test]
        async fn is_refused_and_writes_nothing() {
            // given: the holder is an admin, the sender is not
            let holder = setup_holder(admins_view(&[pid(2)])).await;
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then
            assert_eq!(code, Some(RedeemCode::NotFromAdmin));
            holder
                .then_verdict_among(&[pid(1), pid(2)], IssueVerdict::Unknown)
                .await;
        }
    }

    mod hold_from_an_admin {
        use super::*;

        #[tokio::test]
        async fn an_admin_sender_is_held() {
            // given: the sender and the holder are both admins
            let holder = setup_holder(admins_view(&[pid(1), pid(2)])).await;
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then
            assert_eq!(code, Some(RedeemCode::Ok));
            assert!(holder.record_value().await.is_some());
        }

        #[tokio::test]
        async fn keeps_a_copy_on_behalf_of_the_sender() {
            // given: the sender and the holder are both admins
            let holder = setup_holder(admins_view(&[pid(1), pid(2)])).await;
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then: the copy is the holder's record that names the sender, not the holder, as the issuer
            assert_eq!(code, Some(RedeemCode::Ok));
            holder
                .then_verdict_among(&[pid(1), pid(2)], IssueVerdict::Grant)
                .await;
            holder
                .then_verdict_among(&[pid(2)], IssueVerdict::NotAdmin)
                .await;
        }
    }

    mod hold_on_the_hash_of_the_senders_own_invite {
        use super::*;

        #[tokio::test]
        async fn naming_an_admin_issuer_is_refused_and_does_not_grant() {
            // given: a member wrote the record itself, naming an admin as the issuer
            let holder = setup_holder(admins_view(&[pid(2)])).await;
            let value = issue_value(&issue_record(unix_now(), None), Some(pid(2))).unwrap();
            holder.given_record_by(pid(1), value);
            // when: the same member asks the holder to keep a copy
            let code = holder.when_hold_arrives(unix_now()).await;
            // then: refused, no copy, and the member's own record does not count
            assert_eq!(code, Some(RedeemCode::NotFromAdmin));
            assert_eq!(holder.record_value().await, None);
            holder
                .then_verdict_among(&[pid(2)], IssueVerdict::NotAdmin)
                .await;
        }

        #[tokio::test]
        async fn without_an_issuer_is_refused_and_does_not_grant() {
            // given: a member wrote a plain record
            let holder = setup_holder(admins_view(&[pid(2)])).await;
            let value = postcard::to_stdvec(&issue_record(unix_now(), None)).unwrap();
            holder.given_record_by(pid(1), value);
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then
            assert_eq!(code, Some(RedeemCode::NotFromAdmin));
            assert_eq!(holder.record_value().await, None);
            holder
                .then_verdict_among(&[pid(2)], IssueVerdict::NotAdmin)
                .await;
        }
    }

    mod hold_before_the_admin_list {
        use super::*;

        #[tokio::test]
        async fn gets_not_ready_rather_than_not_from_admin() {
            // given: a Managed holder that has no published list of admins yet
            let holder = setup_holder(AccessView::open()).await;
            // when
            let code = holder.when_hold_arrives(unix_now()).await;
            // then: not ready, nothing written
            assert_eq!(code, Some(RedeemCode::NotReady));
            assert_eq!(holder.record_value().await, None);
        }

        #[tokio::test]
        async fn expired_frame_is_answered_before_not_ready() {
            // given: the same holder, a frame older than the invite lifetime
            let holder = setup_holder(AccessView::open()).await;
            let made = unix_now() - crate::invite::INVITE_TTL_SECS - 10;
            // when
            let code = holder.when_hold_arrives(made).await;
            // then
            assert_eq!(code, Some(RedeemCode::Expired));
        }
    }
}

/// The secret hash the HOLD frames ask to keep.
const HASH: [u8; 32] = [5; 32];

/// A view of the holder (`pid(2)`) that has published the given admins.
fn admins_view(admins: &[PeerId]) -> AccessView {
    let ledger = Votes {
        admitted: admins.iter().copied().collect(),
        excluded: Default::default(),
        admissions: Default::default(),
        admins: admins.iter().copied().collect(),
        closed: None,
    };
    let verdicts = Verdicts {
        me: pid(2),
        ledger: Arc::new(ledger),
        blocks: Default::default(),
    };
    let access = AccessView::open();
    access.publish(verdicts);
    access
}

/// The holder: the server over a filled list, and the shared replica its copy goes to.
struct Holder {
    fx: Fixture,
    shared: Arc<dyn Replica>,
    mem: Arc<MemReplica>,
    list: DomainList,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

async fn setup_holder(access: AccessView) -> Holder {
    let net = MemSyncNet::new();
    let backend = net.backend(SyncCtx {
        secret: secret(2),
        network_seed: [9; 32],
        store_dir: None,
    });
    let mem = backend.replica_mem(REPLICA_NET).unwrap();
    let shared: Arc<dyn Replica> = mem.clone();
    let own: Arc<dyn Replica> = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    let fx = setup(Opts {
        access,
        list: Some(list.clone()),
        ..Opts::default()
    });
    Holder {
        fx,
        shared,
        mem,
        list,
        _keep: (net, backend),
    }
}

impl Holder {
    /// The client `pid(1)` sends a HOLD for `HASH` made at `made_unix`; the code of the reply, `None` — no reply.
    async fn when_hold_arrives(&self, made_unix: u64) -> Option<RedeemCode> {
        let session = self.fx.when_client_connects().await;
        let frame = hold_frame(&NetworkKey::from_bytes(KEY), &HASH, made_unix);
        let raw = session.when_asks_raw(&frame).await;
        raw.and_then(|b| parse_hold_reply(&b))
    }

    /// A record under `HASH` that `author` wrote, as it arrives by sync.
    fn given_record_by(&self, author: PeerId, value: Vec<u8>) {
        self.mem.put_raw(ReplicaEntry {
            author,
            key: format!("{ISSUE_PREFIX}{}", hash_hex_of(&HASH)).into_bytes(),
            value: Bytes::from(value),
            ts_micros: (unix_now() - 5) * 1_000_000,
        });
    }

    /// The value of the holder's own record `invite/<hash>`.
    async fn record_value(&self) -> Option<Vec<u8>> {
        let key = format!("{ISSUE_PREFIX}{}", hash_hex_of(&HASH));
        let entry = self.shared.get(pid(2), key.as_bytes()).await.unwrap();
        entry.map(|e| e.value.to_vec())
    }

    /// The Managed verdict on `HASH` over the records of the given admins.
    async fn then_verdict_among(&self, admins: &[PeerId], want: IssueVerdict) {
        let set: BTreeSet<PeerId> = admins.iter().copied().collect();
        let got = Ledger::of(&self.list)
            .verdict(&hash_hex_of(&HASH), unix_now(), Some(&set))
            .await;
        assert_eq!(
            got, want,
            "the verdict among {admins:?} is not the expected one"
        );
    }
}
