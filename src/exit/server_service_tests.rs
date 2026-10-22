//! Tests for the exit handler on the mem stack (`server.rs`): permission, request frame, byte shuttling,
//! end of connection. The domain address and refusals for closed networks are next to `outbound/`.

use super::*;
use crate::exit::clients::EXIT_CLIENT_LINGER;
use crate::exit::frame::{encode_exit_request, EXIT_CODE_ADDRESS_FORBIDDEN};
use crate::http_min::HttpLimits;
use crate::net::facade::test_support::{pid, secret, within, Spy};
use crate::net::mem::MemNet;
use crate::net::service::EXIT;
use crate::net::{NetStack, NetStackBuilder, StreamPair};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

#[path = "server_service_fixture.rs"]
mod fixture;
use fixture::*;

mod refuses {
    use super::*;

    #[tokio::test]
    async fn non_member_gets_refused() {
        // given: the client is not in `Members`
        let fx = setup(Opts {
            member: false,
            ..Opts::default()
        })
        .await;

        // when: it asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the reply is `EXIT_CODE_REFUSED`
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
    }

    #[tokio::test]
    async fn disabled_exit_gets_refused() {
        // given: the client is in `Members`, but the exit is off
        let fx = setup(Opts {
            enabled: false,
            ..Opts::default()
        })
        .await;

        // when: it asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the same reply
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
    }
}

mod refuses_aaaa {
    use super::*;

    #[tokio::test]
    async fn aaaa_only_name_with_mapped_loopback_gets_code_7() {
        // given: the name has no A and its only AAAA is a mapped loopback
        let fx = setup(Opts::default()).await;
        let mapped = "::ffff:127.0.0.1".parse().expect("an IPv6 address");
        fx.cfg.resolver.seed_cache_dual_for_tests("v6only.test", vec![], vec![mapped]);

        // when: a member asks for the name
        let (_pair, reply) = fx.ask(&request("v6only.test", 80)).await;

        // then: the exit refuses with "address closed" (code 7)
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_ADDRESS_FORBIDDEN]);
    }
}

mod bad_frame {
    use super::*;

    #[tokio::test]
    async fn cut_frame_replies_bad_request() {
        // given: the client sent only half a frame and closed its side
        let fx = setup(Opts::default()).await;
        let frame = request("site.test", 80);

        // when: the server reads the frame to the end of the stream
        let (_pair, reply) = fx.ask_then_close(&frame[..3]).await;

        // then: the reply is `EXIT_CODE_BAD_REQUEST`
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_BAD_REQUEST]);
    }
}

mod ok {
    use super::*;

    #[tokio::test]
    async fn splices_bytes_to_local_site() {
        // given: the domain leads to a local echo
        let fx = setup(Opts {
            site: Site::Echo,
            ..Opts::default()
        })
        .await;

        // when: the client asks for a stream and writes four bytes
        let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
        pair.send.write_all(b"ping").await.expect("client write");
        let mut back = [0u8; 4];
        within("echo", pair.recv.read_exact(&mut back))
            .await
            .expect("echo read");

        // then: the code is `EXIT_CODE_OK` and the bytes came back
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        assert_eq!(&back, b"ping");
    }
}

mod traffic {
    use super::*;

    #[tokio::test]
    async fn exit_handle_counts_through_us() {
        // given: the domain leads to an echo, the exit has a shared speed registry
        let fx = setup(Opts {
            site: Site::Echo,
            ..Opts::default()
        })
        .await;
        let (mut pair, _reply) = fx.ask(&request("site.test", 80)).await;
        pair.send.write_all(&[7u8; 10_000]).await.expect("client write");
        let mut back = [0u8; 10_000];
        within("echo", pair.recv.read_exact(&mut back)).await.expect("echo read");

        // when: we wait at most three seconds
        let seen = tokio::time::timeout(Duration::from_secs(3), async {
            while fx.cfg.traffic.rates_at(Instant::now()).get(&pid(1)).is_none_or(|r| r.through_us_bps == 0) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        // then: the client has "through us", and "through peer" is zero
        assert!(seen.is_ok(), "speed did not appear within 3 s");
        let rate = fx.cfg.traffic.rates_at(Instant::now())[&pid(1)];
        assert_eq!(rate.through_peer_bps, 0);
    }
}

mod conn_close {
    use super::*;

    #[tokio::test]
    async fn ends_handler_and_forgets_clients() {
        // given: an open stream to a silent site, the exit client is registered
        let fx = setup(Opts {
            site: Site::Silent,
            ..Opts::default()
        })
        .await;
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        within("client registered", fx.wait_client_listed()).await;

        // when: the client closes the connection
        fx.conn.close(0, b"done");

        // then: `handle` finishes, the client counter returned to zero, also after the linger
        within("handle finished", fx.handle_ended()).await;
        let later = Instant::now() + EXIT_CLIENT_LINGER + Duration::from_secs(1);
        assert!(
            fx.cfg.clients.active_peers_at(later).is_empty(),
            "no clients left"
        );
    }
}
