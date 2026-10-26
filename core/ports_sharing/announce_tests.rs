//! Tests of the pure part of the announcer: `diff_published` (what to write, what to remove).

use super::*;
use crate::ports_sharing::types::{Audience, Proto};
use crate::transport::net::mem::MemSyncNet;
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::NodeSecret;

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

fn announced(list: &[Published]) -> HashMap<(Proto, u16), PortRecord> {
    list.iter()
        .map(|p| ((p.proto, p.port), PortRecord::from(p)))
        .collect()
}

#[test]
fn new_port_is_written() {
    let (w, r) = diff_published(&[p(80, "")], &HashMap::new());
    assert_eq!(w, vec![p(80, "")]);
    assert!(r.is_empty());
}

#[test]
fn same_record_is_skipped() {
    let (w, r) = diff_published(&[p(80, "a")], &announced(&[p(80, "a")]));
    assert!(w.is_empty() && r.is_empty());
}

#[test]
fn renamed_port_is_rewritten() {
    let (w, r) = diff_published(&[p(80, "b")], &announced(&[p(80, "a")]));
    assert_eq!(w, vec![p(80, "b")]);
    assert!(r.is_empty());
}

#[test]
fn dropped_port_is_removed() {
    let (w, r) = diff_published(&[], &announced(&[p(80, "")]));
    assert!(w.is_empty());
    assert_eq!(r, vec![(Proto::Tcp, 80)]);
}

mod two_protocols {
    use super::*;

    #[test]
    fn udp_of_a_published_tcp_number_is_written() {
        // given: TCP 53 is announced, the list now also has UDP 53
        let list = [p(53, "a"), pp(Proto::Udp, 53, "a")];
        // when: the difference is computed
        let (w, r) = diff_published(&list, &announced(&[p(53, "a")]));
        // then: only the UDP entry is written
        assert_eq!(w, vec![pp(Proto::Udp, 53, "a")]);
        assert!(r.is_empty());
    }

    #[test]
    fn dropped_udp_is_removed_while_tcp_of_that_number_stays() {
        let (w, r) = diff_published(
            &[p(53, "a")],
            &announced(&[p(53, "a"), pp(Proto::Udp, 53, "a")]),
        );
        assert!(w.is_empty());
        assert_eq!(r, vec![(Proto::Udp, 53)]);
    }

    #[test]
    fn dropped_tcp_is_removed_while_udp_of_that_number_stays() {
        let (w, r) = diff_published(
            &[pp(Proto::Udp, 53, "a")],
            &announced(&[p(53, "a"), pp(Proto::Udp, 53, "a")]),
        );
        assert!(w.is_empty());
        assert_eq!(r, vec![(Proto::Tcp, 53)]);
    }

    #[test]
    fn removals_are_ordered_by_protocol_then_port() {
        let (_, r) = diff_published(
            &[],
            &announced(&[
                pp(Proto::Udp, 1, ""),
                p(80, ""),
                p(53, ""),
                pp(Proto::Udp, 65535, ""),
            ]),
        );
        assert_eq!(
            r,
            [
                (Proto::Tcp, 53),
                (Proto::Tcp, 80),
                (Proto::Udp, 1),
                (Proto::Udp, 65535)
            ]
        );
    }

    #[test]
    fn renamed_udp_is_rewritten() {
        let (w, r) = diff_published(
            &[pp(Proto::Udp, 53, "b")],
            &announced(&[pp(Proto::Udp, 53, "a")]),
        );
        assert_eq!(w, vec![pp(Proto::Udp, 53, "b")]);
        assert!(r.is_empty());
    }
}

mod cycle_over_a_document {
    use super::*;

    #[tokio::test]
    async fn udp_publication_is_written_without_local_check() {
        // given: UDP 53 is listed, nothing listens on that number over TCP (a TCP check would say "refused")
        let rig = setup();
        let port = free_tcp_port();
        let table = PublishedPortsTable::new(vec![pp(Proto::Udp, port, "dns")]);
        // when: one announcer cycle runs
        rig.run_cycle(&table).await;
        // then: the entry is in the document, and the row's local state is still "unchecked"
        assert_eq!(rig.doc.own_ports().await.unwrap(), [(Proto::Udp, port)]);
        assert_eq!(table.published_with_state()[0].1, LocalState::Unknown);
    }

    #[tokio::test]
    async fn tcp_publication_gets_its_local_state() {
        // given: TCP port with no listener
        let rig = setup();
        let port = free_tcp_port();
        let table = PublishedPortsTable::new(vec![p(port, "web")]);
        // when: one announcer cycle runs
        rig.run_cycle(&table).await;
        // then: written, and the check marked the row as refused
        assert_eq!(rig.doc.own_ports().await.unwrap(), [(Proto::Tcp, port)]);
        assert_eq!(table.published_with_state()[0].1, LocalState::Refused);
    }

    #[tokio::test]
    async fn unlisted_udp_record_is_removed_tcp_stays() {
        // given: the document holds our TCP 53 and UDP 53, the table lists only TCP 53
        let rig = setup();
        rig.doc.publish(&p(53, "a")).await.unwrap();
        rig.doc.publish(&pp(Proto::Udp, 53, "a")).await.unwrap();
        // when: stale entries are cleaned
        remove_unlisted_records(&rig.doc, &[p(53, "a")])
            .await
            .unwrap();
        // then: only TCP 53 is left
        assert_eq!(rig.doc.own_ports().await.unwrap(), [(Proto::Tcp, 53)]);
    }

    #[tokio::test]
    async fn unlisted_tcp_record_is_removed_udp_stays() {
        let rig = setup();
        rig.doc.publish(&p(53, "a")).await.unwrap();
        rig.doc.publish(&pp(Proto::Udp, 53, "a")).await.unwrap();
        remove_unlisted_records(&rig.doc, &[pp(Proto::Udp, 53, "a")])
            .await
            .unwrap();
        assert_eq!(rig.doc.own_ports().await.unwrap(), [(Proto::Udp, 53)]);
    }

    #[tokio::test]
    async fn udp_dropped_from_the_table_disappears_from_the_document() {
        // given: a cycle announced TCP 53 and UDP 53
        let rig = setup();
        let table = PublishedPortsTable::new(vec![p(53, "a"), pp(Proto::Udp, 53, "a")]);
        rig.run_cycle(&table).await;
        assert_eq!(rig.doc.own_ports().await.unwrap().len(), 2);
        // when: UDP 53 leaves the list and the next cycle runs
        table.set_published(vec![p(53, "a")]);
        rig.run_cycle(&table).await;
        // then: TCP 53 stays published
        assert_eq!(rig.doc.own_ports().await.unwrap(), [(Proto::Tcp, 53)]);
    }
}

// --- setup and helpers ---

struct Rig {
    doc: PortsDoc,
    announced: std::sync::Mutex<HashMap<(Proto, u16), PortRecord>>,
}

/// A node with an empty in-memory ports document.
fn setup() -> Rig {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[3; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    let rep = MemSyncNet::new()
        .backend(ctx)
        .replica_mem(REPLICA_NET)
        .unwrap();
    Rig {
        doc: PortsDoc::new(rep),
        announced: Default::default(),
    }
}

impl Rig {
    /// One announcer cycle; the "announced" memory carries over between calls.
    async fn run_cycle(&self, table: &PublishedPortsTable) {
        let mut announced = self.announced.lock().unwrap().clone();
        let mut cleaned = true;
        cycle(&self.doc, table, &mut announced, &mut cleaned)
            .await
            .unwrap();
        *self.announced.lock().unwrap() = announced;
    }
}

/// A loopback TCP port nobody listens on.
fn free_tcp_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    l.local_addr().unwrap().port()
}
