//! Tests of the watcher and of the recompute behind it (`watch.rs`, `runtime.rs`): a change of the list is
//! published and the refused neighbours are cut; witnesses are written for connected nodes only. The watcher runs
//! in real time (its debounce is 300 ms), so every wait polls with a limit instead of sleeping a fixed time.
//! Nodes: `me` is the own node (seed 1); the others are named by what they do in the test.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{
    admit_key, decode, encode, exclude_key, grant_key, AclKind, AclValue, Via,
};
use crate::members::access::view::{AccessView, Deny};
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::conn::MemConn;
use crate::transport::net::mem::{MemNet, MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{
    PeerConn, PeerId, Replica, ReplicaEntry, SplitTransport, Transport, AUTH,
};
use crate::transport::net_mainline::unix_now;
use std::collections::BTreeMap;

/// How long a condition is given to become true.
const LIMIT: Duration = Duration::from_secs(5);

mod watcher {
    use super::*;

    #[tokio::test]
    async fn first_recompute_publishes_the_pending_view() {
        // given: a view that is still pending and a list with one neighbour
        let mut rig = setup();
        rig.given_admit(rig.me, pid(2));
        assert!(
            rig.view().verdicts().is_none(),
            "the view must start pending"
        );
        // when: the watcher starts
        rig.when_watching();
        // then: the first recompute publishes it, and the handshakes may go on
        eventually("a published view", || rig.view().verdicts().is_some()).await;
        assert_eq!(rig.view().check(pid(2)), Ok(()));
        within(rig.view().ready()).await;
    }

    #[tokio::test]
    async fn remote_exclusion_publishes_and_cuts() {
        // given: three nodes on the list, b an admin, a connected; the watcher has published the view
        let mut rig = setup();
        let (a, b) = (pid(2), pid(3));
        [a, b].iter().for_each(|p| rig.given_admit(rig.me, *p));
        rig.given_grant(b);
        let link = rig.given_connection(2).await;
        rig.when_watching();
        eventually("a published view", || rig.view().verdicts().is_some()).await;
        assert!(
            !link.conn.is_closed(),
            "a listed neighbour must stay connected"
        );
        // when: the admin b excludes a, and the record arrives through the synchronization
        rig.given_exclusion(b, a, 100);
        // then: a is denied and its connection is closed with the reason of the denial
        eventually("the connection of a is cut", || link.conn.is_closed()).await;
        assert_eq!(rig.view().check(a), Err(Deny::Excluded));
        assert_eq!(
            close_info(&link.conn),
            Some((403, Bytes::from_static(b"excluded")))
        );
    }

    #[tokio::test]
    async fn left_node_cuts_everyone() {
        // given: two listed neighbours, both connected; the watcher runs
        let mut rig = setup();
        rig.given_admit(rig.me, pid(2));
        rig.given_admit(rig.me, pid(3));
        let (first, second) = (rig.given_connection(2).await, rig.given_connection(3).await);
        rig.when_watching();
        eventually("a published view", || rig.view().verdicts().is_some()).await;
        // when: I write the exclusion of myself
        rig.when_leaving().await;
        // then: every connection is closed with "left", and nobody passes
        eventually("both connections cut", || {
            first.conn.is_closed() && second.conn.is_closed()
        })
        .await;
        assert_eq!(
            close_info(&first.conn),
            Some((403, Bytes::from_static(b"left")))
        );
        assert_eq!(
            close_info(&second.conn),
            Some((403, Bytes::from_static(b"left")))
        );
        assert_eq!(rig.view().check(pid(2)), Err(Deny::Left));
    }
}

mod witnesses {
    use super::*;

    #[tokio::test]
    async fn witness_admits_only_connected_members() {
        // given: b and d are admins admitted by me, both admitted a and c; a and the unlisted x are connected, c is not
        let rig = setup();
        let (a, b, c, d, x) = (pid(2), pid(3), pid(4), pid(5), pid(9));
        rig.given_admit(rig.me, b);
        rig.given_admit(rig.me, d);
        rig.given_grant(b);
        rig.given_grant(d);
        rig.given_admit(b, a);
        rig.given_admit(b, c);
        rig.given_admit(d, a);
        rig.given_admit(d, c);
        rig.members.add_verified_conn(a);
        rig.members.add_verified_conn(x);
        // when: the verdicts are recomputed
        rig.refresh().await;
        // then: I have written a witness for a, and nothing for c (not connected) or x (not listed)
        assert_eq!(
            rig.my_admits().await,
            [(a, Via::Witness), (b, Via::Invite), (d, Via::Invite)]
                .into_iter()
                .collect()
        );
    }

    #[tokio::test]
    async fn witness_is_written_once() {
        // given: a listed by b, connected
        let rig = setup();
        let (a, b) = (pid(2), pid(3));
        rig.given_admit(rig.me, b);
        rig.given_admit(b, a);
        rig.members.add_verified_conn(a);
        rig.refresh().await;
        let first = rig.my_admit_entry(a).await;
        // when: the recompute runs again
        rig.refresh().await;
        // then: the record is the same one
        assert_eq!(
            rig.my_admit_entry(a).await,
            first,
            "the witness was rewritten"
        );
    }

    #[tokio::test]
    async fn connected_unlisted_is_denied_and_not_recorded() {
        // given: a connected node the list does not know
        let rig = setup();
        let x = pid(9);
        rig.members.add_verified_conn(x);
        // when / then: it is denied, and no record is written for it
        rig.refresh().await;
        assert_eq!(rig.view().check(x), Err(Deny::Unlisted));
        assert!(
            rig.my_admits().await.is_empty(),
            "a record for an unlisted node"
        );
    }

    #[tokio::test]
    async fn excluded_connected_node_gets_no_witness() {
        // given: a is listed by the admins b and c and excluded by b; connected
        let rig = setup();
        let (a, b, c) = (pid(2), pid(3), pid(4));
        [b, c].iter().for_each(|p| rig.given_admit(rig.me, *p));
        [b, c].iter().for_each(|p| rig.given_grant(*p));
        rig.given_admit(b, a);
        rig.given_admit(c, a);
        rig.given_exclusion(b, a, 100);
        rig.members.add_verified_conn(a);
        // when: the verdicts are recomputed
        rig.refresh().await;
        // then: no witness for the excluded node
        assert_eq!(rig.view().check(a), Err(Deny::Excluded));
        assert!(
            !rig.my_admits().await.contains_key(&a),
            "a witness for an excluded node"
        );
    }
}

// --- setup and helpers ---

struct Rig {
    access: Arc<Access>,
    shared: Arc<MemReplica>,
    members: Members,
    me: PeerId,
    watching: Vec<AbortHandle>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// A connection of the own node to the node with a seed; both ends and the transports live as long as it does.
struct Link {
    conn: Arc<dyn PeerConn>,
    _far: Arc<dyn PeerConn>,
    _nodes: [Arc<SplitTransport>; 2],
}

/// The node (seed 1) with a pending view.
fn setup() -> Rig {
    let net = MemSyncNet::new();
    let node = secret(1);
    let ctx = SyncCtx {
        secret: node.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let shared = backend.replica_mem(REPLICA_NET).unwrap();
    let own = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let members = Members::with_access(AccessView::pending());
    let access = Access::from_parts(
        node.peer_id(),
        shared.clone(),
        own,
        members.clone(),
        node.peer_id(),
    );
    Rig {
        access: Arc::new(access),
        shared,
        members,
        me: node.peer_id(),
        watching: Vec::new(),
        _keep: (net, backend),
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.watching.iter().for_each(|h| h.abort());
    }
}

impl Rig {
    fn view(&self) -> AccessView {
        self.members.access()
    }

    fn when_watching(&mut self) {
        let task = crate::node::output::spawn(run_watch(self.access.clone()));
        self.watching.push(task.abort_handle());
    }

    async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }

    /// A record of the list as if it arrived from `author` by synchronization.
    fn given_record(
        &self,
        author: PeerId,
        key: Vec<u8>,
        kind: AclKind,
        subject: PeerId,
        via: Via,
        ts_micros: u64,
    ) {
        let value = AclValue {
            kind,
            subject,
            via,
            made_unix: 111,
            note: String::new(),
        };
        self.shared.put_raw(ReplicaEntry {
            author,
            key,
            value: Bytes::from(encode(&value)),
            ts_micros,
        });
    }

    /// An admission; the creator's come first, those of the administrators after the role was given.
    fn given_admit(&self, author: PeerId, subject: PeerId) {
        let ts_micros = if author == self.me { 1 } else { 3 };
        self.given_record(
            author,
            admit_key(&subject),
            AclKind::Admit,
            subject,
            Via::Invite,
            ts_micros,
        );
    }

    /// The role of an administrator, given to `subject` by the creator (me).
    fn given_grant(&self, subject: PeerId) {
        self.given_record(
            self.me,
            grant_key(&subject),
            AclKind::Grant,
            subject,
            Via::None,
            2,
        );
    }

    /// An exclusion of `subject` written by `author` at `ts_micros`.
    fn given_exclusion(&self, author: PeerId, subject: PeerId, ts_micros: u64) {
        self.given_record(
            author,
            exclude_key(&subject),
            AclKind::Exclude,
            subject,
            Via::None,
            ts_micros,
        );
    }

    async fn when_leaving(&self) {
        let value = AclValue {
            kind: AclKind::Exclude,
            subject: self.me,
            via: Via::None,
            made_unix: unix_now(),
            note: String::new(),
        };
        self.shared
            .set_own(&exclude_key(&self.me), Bytes::from(encode(&value)))
            .await
            .unwrap();
    }

    /// A tracked connection to the node with the seed, which the members also count as connected.
    async fn given_connection(&self, seed: u8) -> Link {
        let net = MemNet::new();
        let (near, far) = (net.transport(&secret(1)), net.transport(&secret(seed)));
        far.serve(&[AUTH]);
        let (dialed, accepted) =
            within(async { tokio::join!(near.connect(&far.local_addr(), AUTH), far.accept()) })
                .await;
        let (_, far_conn) = accepted.expect("the far node must accept");
        let conn = dialed.expect("the connection must open");
        self.members.track_conn(&conn);
        self.members.add_verified_conn(pid(seed));
        Link {
            conn,
            _far: far_conn,
            _nodes: [near, far],
        }
    }

    /// My live admits with their `via`, by subject.
    async fn my_admits(&self) -> BTreeMap<PeerId, Via> {
        let all = self.shared.scan(b"acl/admit/").await.unwrap();
        let mine = all
            .iter()
            .filter(|e| e.author == self.me && !e.is_tombstone());
        mine.filter_map(|e| decode(&e.value).ok())
            .map(|v| (v.subject, v.via))
            .collect()
    }

    async fn my_admit_entry(&self, subject: PeerId) -> Option<ReplicaEntry> {
        self.shared
            .get(self.me, &admit_key(&subject))
            .await
            .unwrap()
    }
}

/// Polls until the condition holds; fails with the name after `LIMIT`.
async fn eventually(what: &str, cond: impl Fn() -> bool) {
    let poll = async {
        while !cond() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    assert!(
        tokio::time::timeout(LIMIT, poll).await.is_ok(),
        "{what} did not happen within {LIMIT:?}"
    );
}

async fn within<T>(fut: impl Future<Output = T>) -> T {
    tokio::time::timeout(LIMIT, fut)
        .await
        .expect("the step took too long")
}

fn close_info(conn: &Arc<dyn PeerConn>) -> Option<(u32, Bytes)> {
    conn.as_any()
        .downcast_ref::<MemConn>()
        .expect("the connection must be mem")
        .close_info()
}

#[cfg(test)]
#[path = "watch_retry_tests.rs"]
mod retry_tests;
