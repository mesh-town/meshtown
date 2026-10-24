//! Tests of the slot's "network closed" channel (`closed_feed.rs`): `feed_closed` forwards the value of the
//! access object into the channel that `NodeCtl::closed()` hands out.
//! Nodes: `me` (seed 1), the root and administrator (2).

// assumed: `feed_closed(shared, access)` is the free function of `closed_feed.rs`, and the file is declared there
// as `#[cfg(test)] #[path = "closed_feed_tests.rs"] mod tests;`.

use std::time::Duration;

use bytes::Bytes;
use tokio::sync::watch;

use super::*;
use crate::config::config_file::Settings;
use crate::members::access::authority::Rules;
use crate::members::access::record::{encode, key_for, AclKind, AclValue, Via};
use crate::members::access::view::AccessView;
use crate::members::membership::Members;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::ReplicaEntry;

mod channel_before_any_feed {
    use super::*;

    #[tokio::test]
    async fn holds_none() {
        let slot = setup_slot();
        assert_eq!(*slot.ctl.closed().borrow(), None);
    }
}

mod feed_started_on_an_open_network {
    use super::*;

    #[tokio::test]
    async fn the_channel_stays_none() {
        // given / when
        let mut slot = setup_slot();
        let net = open_network();
        slot.feed(&net);
        // then: the feed has handed over the current value and it is none
        slot.settle().await;
        assert_eq!(*slot.ctl.closed().borrow(), None);
    }

    #[tokio::test]
    async fn a_later_close_reaches_the_channel() {
        // given: the feed runs over an open network
        let mut slot = setup_slot();
        let net = open_network();
        slot.feed(&net);
        let mut rx = slot.ctl.closed();
        // when: the root closes the network and the list is recomputed
        net.given_close(5);
        net.refresh().await;
        // then: the channel carries the closing
        assert_eq!(wait_for_close(&mut rx).await, closed_by_root(5));
    }
}

mod feed_started_on_a_closed_network {
    use super::*;

    #[tokio::test]
    async fn the_current_closing_is_handed_over_first() {
        // given: the network was closed before the feed started
        let mut slot = setup_slot();
        let net = open_network();
        net.given_close(5);
        net.refresh().await;
        let mut rx = slot.ctl.closed();
        // when
        slot.feed(&net);
        // then
        assert_eq!(wait_for_close(&mut rx).await, closed_by_root(5));
    }
}

mod feed_replaced_by_the_feed_of_another_network {
    use super::*;

    #[tokio::test]
    async fn changes_of_the_old_network_do_not_arrive() {
        // given: the first network is fed, then its feed is aborted and the second one is fed
        let mut slot = setup_slot();
        let old = open_network();
        let handle = slot.feed(&old);
        let fresh = open_network();
        handle.abort();
        slot.feed(&fresh);
        slot.settle().await;
        // when: the old network closes
        old.given_close(5);
        old.refresh().await;
        slot.settle().await;
        // then: the channel does not notice
        assert_eq!(*slot.ctl.closed().borrow(), None);
    }

    #[tokio::test]
    async fn changes_of_the_new_network_arrive() {
        // given: the same as above
        let mut slot = setup_slot();
        let old = open_network();
        let handle = slot.feed(&old);
        let fresh = open_network();
        handle.abort();
        slot.feed(&fresh);
        let mut rx = slot.ctl.closed();
        // when: the new network closes
        fresh.given_close(7);
        fresh.refresh().await;
        // then
        assert_eq!(wait_for_close(&mut rx).await, closed_by_root(7));
    }
}

// --- setup and helpers ---

fn root() -> PeerId {
    pid(2)
}

fn closed_by_root(at_unix: u64) -> Closed {
    Closed {
        by: root(),
        at_unix,
    }
}

struct Slot {
    ctl: Arc<NodeCtl>,
    feeds: Vec<JoinHandle<()>>,
}

fn setup_slot() -> Slot {
    Slot {
        ctl: NodeCtl::new(&Settings::default()),
        feeds: Vec::new(),
    }
}

impl Slot {
    /// Starts a feed of the network's access object into the slot's channel; the slot aborts it on drop.
    fn feed(&mut self, net: &Network) -> tokio::task::AbortHandle {
        let task = feed_closed(self.ctl.ctl_shared.clone(), net.access.clone());
        let abort = task.abort_handle();
        self.feeds.push(task);
        abort
    }

    /// Lets the feed tasks run.
    async fn settle(&self) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.feeds.iter().for_each(|f| f.abort());
    }
}

/// Waits (at most 2 s) for the channel to carry a closing and returns it.
async fn wait_for_close(rx: &mut watch::Receiver<Option<Closed>>) -> Closed {
    let seen = tokio::time::timeout(Duration::from_secs(2), rx.wait_for(|v| v.is_some()))
        .await
        .expect("the closing must reach the channel")
        .expect("the channel must stay open");
    seen.clone().expect("waited for some")
}

struct Network {
    access: Arc<Access>,
    shared: Arc<MemReplica>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// A managed network whose root has admitted me; no close record yet.
fn open_network() -> Network {
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
    let access = Access::from_parts(node.peer_id(), shared.clone(), own, members)
        .with_rules(Rules::Managed { root: root() });
    let network = Network {
        access: Arc::new(access),
        shared,
        _keep: (net, backend),
    };
    network.given_record(1, AclKind::Admit, root(), node.peer_id());
    network
}

impl Network {
    /// A close record of the root at second `at`.
    fn given_close(&self, at: u64) {
        self.given_record(at, AclKind::Close, root(), root());
    }

    fn given_record(&self, at: u64, kind: AclKind, author: PeerId, subject: PeerId) {
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

    async fn refresh(&self) {
        self.access
            .refresh()
            .await
            .expect("the recompute must work on a healthy replica");
    }
}
