//! Connector contract suite: the `connector_contract!` macro and helpers for "Connector → path" cases.
//! Plugging in an adapter: `connector_contract!(fn_without_arguments_returning_impl_ConnectorHarness);`.

use crate::transport::net::connect::{Connector, PacketPath};
use crate::transport::net::contract::{must_finish, ConnectorHarness, SEED_A, SEED_B};
use bytes::Bytes;
use std::io::ErrorKind;
use std::sync::Arc;

/// The smallest datagram that any path must carry (at least for QUIC).
pub(crate) const MIN_PACKET_BYTES: usize = 1200;

/// How many datagrams the "arrives" case sends: on real paths loss is allowed.
const BURST: u8 = 10;

/// A live path A↔B: `a` is the dialer's end, `b` is the acceptor's end.
pub(crate) struct PathPair {
    pub a: Box<dyn PacketPath>,
    pub b: Box<dyn PacketPath>,
}

/// Two nodes of one environment: A and B.
pub(crate) async fn connector_pair(
    h: &impl ConnectorHarness,
) -> (Arc<dyn Connector>, Arc<dyn Connector>) {
    let a = must_finish(h.node(SEED_A)).await;
    let b = must_finish(h.node(SEED_B)).await;
    (a, b)
}

/// `from` dials `to` at its `local_addr()`, `to` accepts the path; both steps run at once.
pub(crate) async fn dial_between(from: &Arc<dyn Connector>, to: &Arc<dyn Connector>) -> PathPair {
    let addr = to.local_addr();
    let (dialed, accepted) = tokio::join!(must_finish(from.dial(&addr)), must_finish(to.listen()));
    PathPair {
        a: dialed.unwrap_or_else(|e| panic!("dial gave no path: {e}")),
        b: accepted.expect("listen is closed, yet a path was awaited"),
    }
}

/// Sends `BURST` distinguishable datagrams; returns what was sent.
pub(crate) async fn burst(from: &dyn PacketPath) -> Vec<Bytes> {
    let sent: Vec<Bytes> = (0..BURST).map(|i| Bytes::from(vec![i; 32])).collect();
    for pkt in &sent {
        must_finish(from.send(pkt.clone()))
            .await
            .expect("send on a live path");
    }
    sent
}

/// Waits for the next datagram of the path.
pub(crate) async fn next_packet(at: &dyn PacketPath) -> Bytes {
    must_finish(at.recv()).await.expect("recv on a live path")
}

/// Kind of the `recv` error on a closed path.
pub(crate) async fn recv_error_kind(at: &dyn PacketPath) -> ErrorKind {
    match must_finish(at.recv()).await {
        Ok(pkt) => panic!("recv returned {} bytes instead of an error", pkt.len()),
        Err(e) => e.kind(),
    }
}

macro_rules! connector_contract {
    ($harness:path) => {
        mod connector {
            use super::*;
            use $crate::transport::net::contract::connector::{
                burst, connector_pair, dial_between, next_packet, recv_error_kind, MIN_PACKET_BYTES,
            };
            use $crate::transport::net::contract::{
                must_finish, pid, ConnectorHarness, SEED_A, SEED_B, SEED_C,
            };

            mod dial {
                use super::*;
                use $crate::transport::net::addr::PeerAddr;

                #[tokio::test]
                async fn dialer_path_names_the_dialed_peer() {
                    // given: two nodes
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    // when: A dials B
                    let link = dial_between(&a, &b).await;
                    // then: the dialer's path is verified by the connector and leads to B
                    assert_eq!(link.a.info().peer, pid(SEED_B), "A's path must lead to B");
                }

                #[tokio::test]
                async fn listener_path_names_the_dialer() {
                    // given: two nodes
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    // when: A dials B, B accepts
                    let link = dial_between(&a, &b).await;
                    // then: B's accepted path leads to A
                    assert_eq!(link.b.info().peer, pid(SEED_A), "B's path must lead to A");
                }

                #[tokio::test]
                async fn address_with_a_foreign_id_is_an_error() {
                    // given: an address with the id of a third node C and B's hints
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    let forged = PeerAddr {
                        id: pid(SEED_C),
                        hints: b.local_addr().hints,
                    };
                    // when: A dials that address
                    let result = must_finish(a.dial(&forged)).await;
                    // then: the path is not opened (the peer's key is wrong)
                    assert!(result.is_err(), "dial to a substituted key returned a path");
                }

                #[tokio::test]
                async fn restarted_peer_is_dialed_at_its_new_address() {
                    // given: B restarted with the same key
                    let h = $harness();
                    let (a, _old_b) = connector_pair(&h).await;
                    let b = must_finish(h.restart(SEED_B)).await;
                    // when: A dials B's new address
                    let link = dial_between(&a, &b).await;
                    // then: there is a path, the peer is the same B
                    assert_eq!(
                        link.a.info().peer,
                        pid(SEED_B),
                        "after the restart the key is the same"
                    );
                }
            }

            mod datagram {
                use super::*;

                #[tokio::test]
                async fn from_dialer_reaches_listener() {
                    // given: a live path A↔B
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    let link = dial_between(&a, &b).await;
                    // when: A sends ten datagrams
                    let sent = burst(link.a.as_ref()).await;
                    // then: B receives at least one of them
                    let got = next_packet(link.b.as_ref()).await;
                    assert!(
                        sent.contains(&got),
                        "B received a datagram that A did not send"
                    );
                }

                #[tokio::test]
                async fn from_listener_reaches_dialer() {
                    // given: a live path A↔B
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    let link = dial_between(&a, &b).await;
                    // when: B sends ten datagrams
                    let sent = burst(link.b.as_ref()).await;
                    // then: A receives at least one of them
                    let got = next_packet(link.a.as_ref()).await;
                    assert!(
                        sent.contains(&got),
                        "A received a datagram that B did not send"
                    );
                }

                #[tokio::test]
                async fn max_packet_fits_a_minimal_datagram() {
                    // given: a live path A↔B
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    // when: we ask both ends for the datagram limit
                    let link = dial_between(&a, &b).await;
                    // then: it is at least 1200 bytes
                    assert!(
                        link.a.max_packet_bytes() >= MIN_PACKET_BYTES,
                        "A's path limit is below 1200"
                    );
                    assert!(
                        link.b.max_packet_bytes() >= MIN_PACKET_BYTES,
                        "B's path limit is below 1200"
                    );
                }
            }

            mod close {
                use super::*;
                use std::io::ErrorKind;

                #[tokio::test]
                async fn closed_connector_stops_listening() {
                    // given: connector A with no incoming paths
                    let h = $harness();
                    let (a, _b) = connector_pair(&h).await;
                    // when: A is closed
                    must_finish(a.close()).await;
                    // then: listen answers "closed"
                    assert!(
                        must_finish(a.listen()).await.is_none(),
                        "listen of a closed connector gave a path"
                    );
                }

                #[tokio::test]
                async fn closing_the_dialer_ends_the_live_path_on_both_sides() {
                    // given: a live path A↔B
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    let link = dial_between(&a, &b).await;
                    // when: the dialer's connector A is closed
                    must_finish(a.close()).await;
                    // then: recv at both ends gives UnexpectedEof
                    assert_eq!(
                        recv_error_kind(link.a.as_ref()).await,
                        ErrorKind::UnexpectedEof
                    );
                    assert_eq!(
                        recv_error_kind(link.b.as_ref()).await,
                        ErrorKind::UnexpectedEof
                    );
                }

                #[tokio::test]
                async fn closing_the_listener_ends_the_live_path_on_both_sides() {
                    // given: a live path A↔B
                    let h = $harness();
                    let (a, b) = connector_pair(&h).await;
                    let link = dial_between(&a, &b).await;
                    // when: the acceptor's connector B is closed
                    must_finish(b.close()).await;
                    // then: recv at both ends gives UnexpectedEof
                    assert_eq!(
                        recv_error_kind(link.a.as_ref()).await,
                        ErrorKind::UnexpectedEof
                    );
                    assert_eq!(
                        recv_error_kind(link.b.as_ref()).await,
                        ErrorKind::UnexpectedEof
                    );
                }
            }
        }
    };
}

pub(crate) use connector_contract;
