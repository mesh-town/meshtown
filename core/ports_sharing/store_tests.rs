//! Tests of reading and writing publications in an in-memory replica (`store.rs`).

use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::{NodeSecret, PeerId, Replica, ReplicaEntry};
use bytes::Bytes;
use std::sync::Arc;

use super::*;
use crate::ports_sharing::types::Audience;

mod own_port {
    use super::*;

    #[tokio::test]
    async fn put_is_read_back_with_record() {
        // given: an empty replica
        let r = setup();
        // when: the node publishes port 8080 named "web"
        r.doc.publish(&p(8080, "web")).await.unwrap();
        // then: the overview has one entry of this node with a parsed value, and the port counts as our own
        let views = r.doc.read_all().await.unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!((views[0].author, views[0].port), (r.me, 8080));
        let rec = views[0].record.clone().unwrap();
        assert_eq!((rec.v, rec.name.as_str()), (1, "web"));
        assert_eq!(r.doc.own_ports().await.unwrap(), [8080]);
    }
}

mod remove {
    use super::*;

    #[tokio::test]
    async fn leaves_no_view_and_no_own_port() {
        // given: port 8080 is published
        let r = setup();
        r.doc.publish(&p(8080, "")).await.unwrap();
        // when: the publication is removed
        r.doc.unpublish(8080).await.unwrap();
        // then: the tombstone is visible neither in the overview nor in our own ports
        assert!(r.doc.read_all().await.unwrap().is_empty());
        assert!(r.doc.own_ports().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn keeps_longer_port_with_same_prefix() {
        // given: ports 80 and 8080 are published (key "80" is the start of key "8080")
        let r = setup();
        r.doc.publish(&p(80, "")).await.unwrap();
        r.doc.publish(&p(8080, "")).await.unwrap();
        // when: port 80 is removed
        r.doc.unpublish(80).await.unwrap();
        // then: port 8080 is still there
        assert_eq!(r.doc.own_ports().await.unwrap(), [8080]);
    }
}

mod bad_value {
    use super::*;

    #[tokio::test]
    async fn junk_bytes_give_closed_record_but_port_stays_listed() {
        // given: the replica holds unreadable bytes under the key of port 81
        let r = setup();
        r.set_own_raw(&port_key(81), &[0xFF, 0xFF]).await;
        // when: we read the overview
        let views = r.doc.read_all().await.unwrap();
        // then: the entry is there, but without a parsed value; the port counts as our own
        assert_eq!(
            (views.len(), views[0].port, views[0].record.clone()),
            (1, 81, None)
        );
        assert!(r.doc.own_ports().await.unwrap().contains(&81));
    }

    #[tokio::test]
    async fn malformed_key_is_skipped() {
        // given: under a key with no trailing "/" and a leading zero lies a valid value
        let r = setup();
        let value = encode_port_record(&PortRecord::from(&p(80, "")));
        r.set_own_raw(b"port/tcp/080/", &value).await;
        // when: we read the overview and our own ports
        // then: it is as if the entry did not exist
        assert!(r.doc.read_all().await.unwrap().is_empty());
        assert!(r.doc.own_ports().await.unwrap().is_empty());
    }
}

mod foreign_author {
    use super::*;

    #[tokio::test]
    async fn is_seen_after_own_sorted_by_author_bytes() {
        // given: our own port 80 and port 9000 from a second author, which arrived by reconciliation
        let r = setup();
        r.doc.publish(&p(80, "")).await.unwrap();
        let other = peer(4);
        r.put_foreign(
            other,
            &port_key(9000),
            &encode_port_record(&PortRecord::from(&p(9000, "t"))),
        );
        // when: we read the overview
        let views = r.doc.read_all().await.unwrap();
        // then: two entries, ordered by author bytes; the foreign port is parsed
        assert_eq!(views.len(), 2);
        let mut ids = [r.me, other];
        ids.sort_by_key(|i| *i.as_bytes());
        assert_eq!([views[0].author, views[1].author], ids);
        let their = views.iter().find(|v| v.author == other).unwrap();
        assert_eq!(their.port, 9000);
        assert_eq!(their.record.as_ref().map(|x| x.name.as_str()), Some("t"));
    }

    #[tokio::test]
    async fn tombstone_leaves_no_view() {
        // given: only a tombstone for a port key arrived from a second author
        let r = setup();
        r.put_foreign(peer(4), &port_key(9000), b"");
        // when: we read the overview
        // then: empty
        assert!(r.doc.read_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn port_is_not_counted_as_own() {
        // given: the second author has port 9000 published, we have 80
        let r = setup();
        r.doc.publish(&p(80, "")).await.unwrap();
        r.put_foreign(
            peer(4),
            &port_key(9000),
            &encode_port_record(&PortRecord::from(&p(9000, ""))),
        );
        // when: we ask for our own ports
        // then: only 80
        assert_eq!(r.doc.own_ports().await.unwrap(), [80]);
    }
}

// --- setup and helpers ---

struct Rig {
    doc: PortsDoc,
    rep: Arc<MemReplica>,
    me: PeerId,
}

/// A node with author `[3; 32]`, its shared in-memory replica and the ports document over it.
fn setup() -> Rig {
    let secret = NodeSecret::from_bytes(&[3; 32]);
    let ctx = SyncCtx {
        secret: secret.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    let rep = backend.replica_mem(REPLICA_NET).unwrap();
    Rig {
        doc: PortsDoc::new(rep.clone()),
        rep,
        me: secret.peer_id(),
    }
}

impl Rig {
    /// Our own entry with arbitrary value bytes (bypassing `PortsDoc`).
    async fn set_own_raw(&self, key: &[u8], value: &[u8]) {
        self.rep
            .set_own(key, Bytes::copy_from_slice(value))
            .await
            .unwrap();
    }

    /// An entry of a foreign author, as arrived by reconciliation; an empty value is a tombstone.
    fn put_foreign(&self, author: PeerId, key: &[u8], value: &[u8]) {
        self.rep.put_raw(ReplicaEntry {
            author,
            key: key.to_vec(),
            value: Bytes::copy_from_slice(value),
            ts_micros: 1_000_000,
        });
    }
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

fn p(port: u16, name: &str) -> Published {
    Published {
        port,
        name: name.into(),
        audience: Audience::All,
    }
}
