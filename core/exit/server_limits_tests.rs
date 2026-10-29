//! Tests of the cap on live exit streams on the mem stack (`server.rs`): which requests take a place, what a refused
//! one gets, when places come back. The fixture is the one of `server_service_tests.rs`.

use super::*;
use crate::exit::frame::{EXIT_CODE_BAD_REQUEST, EXIT_CODE_OK};
use crate::exit::windowed_copy::stub::pattern;
use crate::exit::{ExitLimits, UDPGW_HOST, UDPGW_PORT};
use crate::transport::net::facade::test_support::within;
use crate::transport::net::StreamPair;
use std::time::Duration;

use super::service_fixture::*;

const BUSY: [u8; 1] = [EXIT_PROTOCOL_VERSION];
const OPENED: [u8; 2] = [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK];

mod limits {
    use super::*;

    mod one_slot {
        use super::*;

        #[tokio::test]
        async fn first_stream_opens() {
            // given: a cap of 1 and an echo site
            let fx = setup_with(1, Site::Echo).await;

            // when: the first stream is asked for and a few bytes go through
            let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
            let back = echo(&mut pair, b"ping").await;

            // then: it opens and the bytes come back
            assert_eq!(reply, OPENED);
            assert_eq!(&back, b"ping");
        }

        #[tokio::test]
        async fn second_stream_gets_busy_without_dialing() {
            // given: a cap of 1 and its stream open
            let fx = setup_with(1, Site::Silent).await;
            let (_first, reply) = fx.ask(&request("site.test", 80)).await;
            assert_eq!(reply, OPENED);

            // when: a second stream is asked for
            let (_second, reply) = fx.ask_raw(&request("site.test", 80)).await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // then: the short reply, and the site saw only the first connection
            assert_eq!(reply, BUSY);
            assert_eq!(fx.site_accepts(), 1, "a refused stream must not dial");
        }
    }

    #[tokio::test]
    async fn closing_a_stream_frees_its_slot() {
        // given: a cap of 1 and its stream open on an echo site
        let fx = setup_with(1, Site::Echo).await;
        let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);
        assert_eq!(fx.cfg.limits.live(), 1);

        // when: the client closes its side and the site's end follows
        pair.send.shutdown().await.expect("client side closed");
        let mut rest = Vec::new();
        within("stream end", pair.recv.read_to_end(&mut rest))
            .await
            .expect("end of stream");

        // then: the place comes back and a new stream opens
        wait_live(&fx, 0, Duration::from_secs(3)).await;
        let (_next, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);
    }

    #[tokio::test]
    async fn closing_the_connection_aborts_streams_and_frees_slots() {
        // given: an echo stream that carried bytes (so the splice would wait for the long grace)
        let fx = setup_with(2, Site::Echo).await;
        let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);
        echo(&mut pair, b"ping").await;
        assert_eq!(fx.cfg.limits.live(), 1);

        // when: the client closes the whole connection
        fx.conn.close(0, b"done");

        // then: the exit drops the stream at once and frees the place
        wait_live(&fx, 0, Duration::from_secs(2)).await;
    }

    mod full {
        use super::*;

        #[tokio::test]
        async fn non_member_still_gets_refused() {
            // given: every place taken and the client is not a member
            let fx = setup_full(Opts {
                member: false,
                ..full_opts()
            })
            .await;

            // when: it asks
            let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

            // then: the usual refusal, not "busy"
            assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
        }

        #[tokio::test]
        async fn disabled_exit_still_gets_refused() {
            // given: every place taken and the exit is off
            let fx = setup_full(Opts {
                enabled: false,
                ..full_opts()
            })
            .await;

            // when: a member asks
            let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

            // then: the usual refusal
            assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
        }

        #[tokio::test]
        async fn cut_frame_still_gets_bad_request() {
            // given: every place taken
            let fx = setup_full(full_opts()).await;
            let frame = request("site.test", 80);

            // when: the client sends half a frame and closes its side
            let (_pair, reply) = fx.ask_then_close(&frame[..3]).await;

            // then: the usual bad-request reply
            assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_BAD_REQUEST]);
        }

        #[tokio::test]
        async fn gateway_target_gets_busy() {
            // given: every place taken
            let fx = setup_full(full_opts()).await;

            // when: the UDP gateway target is asked for
            let (_pair, reply) = fx.ask_raw(&request(UDPGW_HOST, UDPGW_PORT)).await;

            // then: the short reply
            assert_eq!(reply, BUSY);
        }

        #[tokio::test]
        async fn busy_reply_is_the_version_byte_then_eof() {
            // given: every place taken
            let fx = setup_full(full_opts()).await;

            // when: an ordinary target is asked for
            let (_pair, reply) = fx.ask_raw(&request("site.test", 80)).await;

            // then: exactly the version byte, and the stream ended (`ask_raw` stops early only on the end)
            assert_eq!(reply, BUSY);
        }
    }

    mod live {
        use super::*;

        #[tokio::test]
        async fn lowering_the_limit_keeps_open_streams() {
            // given: a cap of 2 with two echo streams open
            let fx = setup_with(2, Site::Echo).await;
            let (mut first, _) = fx.ask(&request("site.test", 80)).await;
            let (mut second, _) = fx.ask(&request("site.test", 80)).await;

            // when: the cap drops to 1
            fx.cfg.limits.set_max_streams(1);

            // then: both open streams still carry bytes, a third is refused
            assert_eq!(&echo(&mut first, b"one!").await, b"one!");
            assert_eq!(&echo(&mut second, b"two!").await, b"two!");
            let (_third, reply) = fx.ask_raw(&request("site.test", 80)).await;
            assert_eq!(reply, BUSY);
        }
    }
}

mod window {
    use super::*;

    #[tokio::test]
    async fn a_windowed_exit_stream_echoes_byte_exact() {
        // given: an exit with a window of 64 KiB and an echo site
        let limits = ExitLimits::fixed(10);
        limits.configure_window(64, 0);
        let fx = setup(Opts {
            site: Site::Echo,
            limits,
            ..Opts::default()
        })
        .await;
        let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);

        // when: 128 KiB (twice the window, within the transport's own) go there and back
        let bytes = pattern(128 * 1024);
        let back = echo(&mut pair, &bytes).await;

        // then: the same bytes come back
        assert!(back == bytes, "the echoed bytes differ from the sent ones");
    }

    #[tokio::test]
    async fn changing_the_window_keeps_open_streams_flowing() {
        // given: an exit without a window and a stream open on it
        let fx = setup_with(10, Site::Echo).await;
        let (mut first, _) = fx.ask(&request("site.test", 80)).await;

        // when: the window is switched on
        fx.cfg.limits.set_stream_window(64);
        assert_eq!(fx.cfg.limits.stream_window_bytes(), 65_536);

        // then: the open stream still carries bytes, and a new one opens and carries bytes too
        assert_eq!(&echo(&mut first, b"one!").await, b"one!");
        let (mut second, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, OPENED);
        assert_eq!(&echo(&mut second, b"two!").await, b"two!");
    }
}

// ---- setup and helpers ----

/// An exit with the cap `max_streams` and the given kind of site.
async fn setup_with(max_streams: u64, site: Site) -> Fx {
    setup(Opts {
        site,
        limits: ExitLimits::fixed(max_streams),
        ..Opts::default()
    })
    .await
}

/// Preconditions of the `full` group: a cap of 1 and a silent site.
fn full_opts() -> Opts {
    Opts {
        limits: ExitLimits::fixed(1),
        ..Opts::default()
    }
}

/// An exit whose only place is taken by someone else (the slot is held by the returned fixture's limiter).
async fn setup_full(opts: Opts) -> Fx {
    let mut fx = setup(opts).await;
    let slot = fx.cfg.limits.try_admit().expect("the only place");
    fx.hold(slot);
    fx
}

/// Writes `bytes` into the stream and reads the same number back.
async fn echo(pair: &mut StreamPair, bytes: &[u8]) -> Vec<u8> {
    pair.send.write_all(bytes).await.expect("client write");
    let mut back = vec![0u8; bytes.len()];
    within("echo", pair.recv.read_exact(&mut back))
        .await
        .expect("echo read");
    back
}

/// Waits until the exit has exactly `n` live streams.
async fn wait_live(fx: &Fx, n: u64, limit: Duration) {
    let reached = tokio::time::timeout(limit, async {
        while fx.cfg.limits.live() != n {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        reached.is_ok(),
        "live streams stayed at {}",
        fx.cfg.limits.live()
    );
}
