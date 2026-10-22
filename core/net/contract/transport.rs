//! Contract suite of the transport: the `transport_contract!` macro; case helpers are in `transport_cases`.
//! Plugging in an adapter: `transport_contract!(fn_without_arguments_returning_impl_Harness);`.

macro_rules! transport_contract {
    ($harness:path) => {
        mod transport {
            use super::*;
            use $crate::net::contract::transport_cases::{connect_between, transport_pair};
            use $crate::net::contract::{pid, must_finish, Harness, SEED_A, SEED_B, SEED_C};
            use $crate::net::service::AUTH;

            mod connect {
                use super::*;
                use $crate::net::addr::PeerAddr;

                #[tokio::test]
                async fn accept_yields_the_dialed_service() {
                    // given: two nodes, B serves AUTH and PORT
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    // when: A connects to B on AUTH
                    let (svc, _pair) = connect_between(&a, &b, AUTH).await;
                    // then: B accepted the connection on AUTH
                    assert_eq!(svc, AUTH, "accept returned the wrong service");
                }

                #[tokio::test]
                async fn acceptor_conn_names_the_dialer() {
                    // given: two nodes
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    // when: A connects to B
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // then: the accepted connection names A
                    assert_eq!(pair.b.peer(), a.me(), "B's counterpart must be A");
                }

                #[tokio::test]
                async fn dialer_conn_names_the_acceptor() {
                    // given: two nodes
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    // when: A connects to B
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // then: the dialer's connection names B
                    assert_eq!(pair.a.peer(), b.me(), "A's counterpart must be B");
                }

                #[tokio::test]
                async fn address_with_a_foreign_id_is_an_error() {
                    // given: an address with the id of a third node C and the hints of B
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let forged = PeerAddr {
                        id: pid(SEED_C),
                        hints: b.local_addr().hints,
                    };
                    // when: A connects to that address
                    let result = must_finish(a.connect(&forged, AUTH)).await;
                    // then: there is no connection (the counterpart key is wrong)
                    assert!(
                        result.is_err(),
                        "connect to a substituted key returned a connection"
                    );
                }

                #[tokio::test]
                async fn restarted_peer_is_connected_at_its_new_address() {
                    // given: B is restarted with the same key
                    let h = $harness();
                    let (a, _old_b) = transport_pair(&h).await;
                    let b = must_finish(h.restart(SEED_B)).await;
                    // when: A connects to the new address of B
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // then: there is a connection, the counterpart is the same B
                    assert_eq!(pair.a.peer(), pid(SEED_B), "the key is the same after the restart");
                }
            }

            mod unserved_service {
                use super::*;
                use $crate::net::contract::transport_cases::{assert_not_accepted, rejected};
                use $crate::net::service::EXIT;

                #[tokio::test]
                async fn dialer_is_refused_or_closed_within_two_seconds() {
                    // given: B serves AUTH and PORT
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    // when: A connects to B on EXIT
                    let outcome = must_finish(a.connect(&b.local_addr(), EXIT)).await;
                    // then: connect is an error, or the connection is closed within 2 s
                    assert!(
                        rejected(outcome).await,
                        "EXIT without a service was not rejected"
                    );
                }

                #[tokio::test]
                async fn acceptor_never_hands_it_over() {
                    // given: B serves AUTH and PORT
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    // when: A connects to B on EXIT
                    let _outcome = must_finish(a.connect(&b.local_addr(), EXIT)).await;
                    // then: accept at B does not return EXIT
                    assert_not_accepted(&b, EXIT).await;
                }
            }

            mod streams {
                use super::*;
                use $crate::net::contract::transport_cases::{
                    echo_mismatches, ping_pong, spawn_echo, transfer_volume, PING, PONG, STREAMS,
                    STREAM_BYTES,
                };

                #[tokio::test]
                async fn hundred_streams_echo_every_byte() {
                    // given: connection A→B, B returns everything written to its streams
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    let _echo = spawn_echo(pair.b.clone());
                    // when: A opens 100 streams at once and sends 64 KiB into each
                    let bad = echo_mismatches(&pair.a, STREAMS, STREAM_BYTES, h.hundred_streams_limit()).await;
                    // then: the echo of every stream matches what was sent
                    assert!(bad.is_empty(), "echo of streams {bad:?} did not match");
                }

                #[tokio::test]
                async fn half_close_lets_the_reply_through() {
                    // given: connection A→B
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // when: A writes "ping" and half-closes, B reads and answers "pong"
                    let (heard, answer) = ping_pong(&pair).await;
                    // then: B read "ping" to the end of the stream, and "pong" reached A
                    assert_eq!(heard, PING, "B did not hear \"ping\" to the end of the stream");
                    assert_eq!(answer, PONG, "what reached A is not \"pong\"");
                }

                #[tokio::test]
                async fn sixty_four_mebibytes_arrive_intact() {
                    // given: connection A→B
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // when: A sends 64 MiB of pseudo-random bytes in one stream
                    let (sent, got) = transfer_volume(&pair).await;
                    // then: the length and blake3 of what was received match what was sent
                    assert_eq!(got.len, sent.len, "received not as much as was sent");
                    assert_eq!(got.hash, sent.hash, "blake3 of the received data did not match");
                }
            }

            mod close {
                use super::*;
                use $crate::net::contract::must_finish_within;
                use $crate::net::contract::transport_cases::{
                    becomes_closed, is_closed_error, ConnPair, CLOSE_WITHIN,
                };

                #[tokio::test]
                async fn closing_ends_the_peer_side_within_two_seconds() {
                    // given: connection A→B
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    // when: A closes the connection with code 7
                    pair.a.close(7, b"bye");
                    // then: closed() at B fires within 2 s
                    must_finish_within(CLOSE_WITHIN, pair.b.closed()).await;
                }

                #[tokio::test]
                async fn accept_after_close_is_closed() {
                    // given: connection A→B, closed from the side of A
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    pair.a.close(7, b"bye");
                    must_finish_within(CLOSE_WITHIN, pair.b.closed()).await;
                    // when: B waits for an incoming stream
                    let result = must_finish(pair.b.accept()).await;
                    // then: Err(Closed)
                    assert!(
                        is_closed_error(&result),
                        "accept on a closed connection is not Closed"
                    );
                }

                #[tokio::test]
                async fn dropping_the_last_handle_closes_the_peer_within_two_seconds() {
                    // given: connection A→B, A has one handle
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (
                        _,
                        ConnPair {
                            a: held_by_a,
                            b: held_by_b,
                        },
                    ) = connect_between(&a, &b, AUTH).await;
                    // when: A drops all handles of the connection
                    drop(held_by_a);
                    // then: closed() at B fires within 2 s
                    must_finish_within(CLOSE_WITHIN, held_by_b.closed()).await;
                }

                #[tokio::test]
                async fn is_closed_turns_true_after_close() {
                    // given: connection A→B, both handles still open
                    let h = $harness();
                    let (a, b) = transport_pair(&h).await;
                    let (_, pair) = connect_between(&a, &b, AUTH).await;
                    assert!(
                        !pair.a.is_closed() && !pair.b.is_closed(),
                        "a new connection is already considered closed"
                    );
                    // when: A closes the connection with code 7
                    pair.a.close(7, b"x");
                    // then: is_closed() at A is true at once, at B within STEP
                    assert!(pair.a.is_closed(), "is_closed() of the closing side is not true at once");
                    assert!(
                        becomes_closed(&pair.b).await,
                        "is_closed() of the counterpart did not become true within STEP"
                    );
                }
            }

            mod info {
                use super::*;
                use $crate::net::contract::transport_cases::link_after_exchange;
                use $crate::net::link::PathKind;

                #[tokio::test]
                async fn link_path_is_known_after_an_exchange() {
                    // given: A and B exchanged "ping" and "pong"
                    let h = $harness();
                    // when: we ask link() at A
                    let link = link_after_exchange(&h).await;
                    // then: the path is known
                    assert!(
                        !matches!(link.path, PathKind::Unknown),
                        "path after the exchange is Unknown"
                    );
                }

                #[tokio::test]
                async fn link_counts_sent_bytes_after_an_exchange() {
                    // given: A and B exchanged "ping" and "pong"
                    let h = $harness();
                    // when: we ask link() at A
                    let link = link_after_exchange(&h).await;
                    // then: the sent bytes are counted
                    assert!(link.sent_bytes > 0, "sent_bytes after the exchange is zero");
                }

                #[tokio::test]
                async fn local_addr_carries_the_own_id() {
                    // given: node A
                    let h = $harness();
                    let (a, _b) = transport_pair(&h).await;
                    // when: we ask for its own address
                    let addr = a.local_addr();
                    // then: the id in it is the id of the node itself
                    assert_eq!(addr.id, a.me(), "local_addr().id is not equal to me()");
                }

                #[tokio::test]
                async fn me_is_the_id_of_the_harness_secret() {
                    // given: node A with the key from the seed SEED_A
                    let h = $harness();
                    let (a, _b) = transport_pair(&h).await;
                    // when: we ask me()
                    // then: it is the id of the key from the seed
                    assert_eq!(a.me(), pid(SEED_A), "me() is not the id of the environment key");
                }
            }
        }
    };
}

pub(crate) use transport_contract;
