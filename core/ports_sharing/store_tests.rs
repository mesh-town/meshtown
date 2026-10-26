//! Tests of reading and writing publications in an in-memory replica (`store.rs`).

use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::{NodeSecret, PeerId, Replica, ReplicaEntry};
use bytes::Bytes;
use std::sync::Arc;

use super::*;
use crate::ports_sharing::types::{Audience, Proto};

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
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Tcp, 8080)]);
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
        r.doc.unpublish(Proto::Tcp, 8080).await.unwrap();
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
        r.doc.unpublish(Proto::Tcp, 80).await.unwrap();
        // then: port 8080 is still there
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Tcp, 8080)]);
    }
}

mod bad_value {
    use super::*;

    #[tokio::test]
    async fn junk_bytes_give_closed_record_but_port_stays_listed() {
        // given: the replica holds unreadable bytes under the key of port 81
        let r = setup();
        r.set_own_raw(&port_key(Proto::Tcp, 81), &[0xFF, 0xFF])
            .await;
        // when: we read the overview
        let views = r.doc.read_all().await.unwrap();
        // then: the entry is there, but without a parsed value; the port counts as our own
        assert_eq!(
            (views.len(), views[0].port, views[0].record.clone()),
            (1, 81, None)
        );
        assert!(r.doc.own_ports().await.unwrap().contains(&(Proto::Tcp, 81)));
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
            &port_key(Proto::Tcp, 9000),
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
        r.put_foreign(peer(4), &port_key(Proto::Tcp, 9000), b"");
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
            &port_key(Proto::Tcp, 9000),
            &encode_port_record(&PortRecord::from(&p(9000, ""))),
        );
        // when: we ask for our own ports
        // then: only 80
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Tcp, 80)]);
    }
}

mod two_protocols {
    use super::*;

    #[tokio::test]
    async fn read_all_returns_both_protocols() {
        // given: TCP 53 and UDP 53 of one node are published
        let r = setup();
        r.doc.publish(&pp(Proto::Udp, 53, "dns-udp")).await.unwrap();
        r.doc.publish(&pp(Proto::Tcp, 53, "dns-tcp")).await.unwrap();
        // when: we read the overview
        let views = r.doc.read_all().await.unwrap();
        // then: two entries, TCP first for the same port, each with its own value
        let rows: Vec<(Proto, u16, Option<String>)> = views
            .iter()
            .map(|v| (v.proto, v.port, v.record.as_ref().map(|x| x.name.clone())))
            .collect();
        assert_eq!(
            rows,
            [
                (Proto::Tcp, 53, Some("dns-tcp".to_string())),
                (Proto::Udp, 53, Some("dns-udp".to_string()))
            ]
        );
    }

    #[tokio::test]
    async fn own_ports_lists_pairs_of_both_protocols() {
        // given: UDP 53, TCP 80 and TCP 53 are published
        let r = setup();
        r.doc.publish(&pp(Proto::Udp, 53, "")).await.unwrap();
        r.doc.publish(&p(80, "")).await.unwrap();
        r.doc.publish(&p(53, "")).await.unwrap();
        // when: we ask for our own ports
        // then: pairs, ordered by protocol and then port
        assert_eq!(
            r.doc.own_ports().await.unwrap(),
            [(Proto::Tcp, 53), (Proto::Tcp, 80), (Proto::Udp, 53)]
        );
    }

    #[tokio::test]
    async fn udp_record_is_written_under_the_udp_key() {
        // given: an empty replica
        let r = setup();
        // when: UDP 53 is published
        r.doc.publish(&pp(Proto::Udp, 53, "dns")).await.unwrap();
        // then: the document holds exactly the key `port/udp/53/`
        let entries = r.rep.scan(b"port/").await.unwrap();
        let keys: Vec<Vec<u8>> = live(entries).into_iter().map(|e| e.key).collect();
        assert_eq!(keys, [b"port/udp/53/".to_vec()]);
    }

    #[tokio::test]
    async fn tcp_prefix_scan_skips_udp_records() {
        // given: only a UDP record is published (what an older node would scan for)
        let r = setup();
        r.doc.publish(&pp(Proto::Udp, 53, "dns")).await.unwrap();
        // when: the old prefix `port/tcp/` is scanned
        let entries = r
            .rep
            .scan(port_prefix(Proto::Tcp).as_bytes())
            .await
            .unwrap();
        // then: nothing is found
        assert!(
            entries.is_empty(),
            "a UDP record is visible under port/tcp/"
        );
    }

    #[tokio::test]
    async fn unpublishing_udp_keeps_tcp_of_the_same_number() {
        // given: TCP 53 and UDP 53 are published
        let r = setup();
        r.doc.publish(&p(53, "")).await.unwrap();
        r.doc.publish(&pp(Proto::Udp, 53, "")).await.unwrap();
        // when: UDP 53 is removed
        r.doc.unpublish(Proto::Udp, 53).await.unwrap();
        // then: TCP 53 is still there, UDP 53 is gone
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Tcp, 53)]);
        let views = r.doc.read_all().await.unwrap();
        assert_eq!(
            views.iter().map(|v| v.proto).collect::<Vec<_>>(),
            [Proto::Tcp]
        );
    }

    #[tokio::test]
    async fn unpublishing_tcp_keeps_udp_of_the_same_number() {
        let r = setup();
        r.doc.publish(&p(53, "")).await.unwrap();
        r.doc.publish(&pp(Proto::Udp, 53, "")).await.unwrap();
        r.doc.unpublish(Proto::Tcp, 53).await.unwrap();
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Udp, 53)]);
    }

    #[tokio::test]
    async fn foreign_udp_record_is_seen_with_its_protocol() {
        // given: a second author has UDP 9000 published, we have nothing
        let r = setup();
        let other = peer(4);
        r.put_foreign(
            other,
            &port_key(Proto::Udp, 9000),
            &encode_port_record(&PortRecord::from(&pp(Proto::Udp, 9000, "t"))),
        );
        // when: we read the overview and our own ports
        let views = r.doc.read_all().await.unwrap();
        // then: one UDP entry of that author, and nothing counts as ours
        assert_eq!(
            (views[0].author, views[0].port, views[0].proto),
            (other, 9000, Proto::Udp)
        );
        assert!(r.doc.own_ports().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn junk_udp_value_keeps_the_port_listed_without_a_record() {
        let r = setup();
        r.set_own_raw(&port_key(Proto::Udp, 81), &[0xFF, 0xFF])
            .await;
        let views = r.doc.read_all().await.unwrap();
        assert_eq!(
            (
                views.len(),
                views[0].proto,
                views[0].port,
                views[0].record.clone()
            ),
            (1, Proto::Udp, 81, None)
        );
        assert_eq!(r.doc.own_ports().await.unwrap(), [(Proto::Udp, 81)]);
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
    pp(Proto::Tcp, port, name)
}

fn pp(proto: Proto, port: u16, name: &str) -> Published {
    Published {
        port,
        proto,
        name: name.into(),
        audience: Audience::All,
    }
}
