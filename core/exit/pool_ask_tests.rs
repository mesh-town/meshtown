//! Tests of `ask_exit_to_connect` and a hung stream open (`pool.rs`) on the mem stack: texts of reply and request errors,
//! `StreamBusy`. The exit server is a scripted handler that answers with the given bytes.
//! A "tricky" connection is planted directly into the pool cell (`PeerConnPool.conns`, `Slot`).

use super::*;
use crate::exit::frame::{EXIT_CODE_OK, EXIT_PROTOCOL_VERSION};
use crate::exit::test_support::Scripted;
use crate::transport::net::facade::test_support::{pid, secret, within, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::EXIT;
use crate::transport::net::{
    BoxFuture, LinkInfo, NetError, NetResult, NetStack, NetStackBuilder, PeerConn, StreamPair,
};
use std::sync::atomic::AtomicU64;
use tokio::io::duplex;

mod reply {
    use super::*;

    #[tokio::test]
    async fn with_foreign_version_is_err_naming_the_version() {
        // given: the exit answers with a version other than `EXIT_PROTOCOL_VERSION`
        let fx = setup(&[EXIT_PROTOCOL_VERSION + 1, EXIT_CODE_OK]);

        // when: the client asks for a stream
        let text = fx.ask_error().await;

        // then: the error names the unsupported version
        assert!(
            text.contains("unsupported exit reply version"),
            "got: {text}"
        );
    }

    #[tokio::test]
    async fn closed_before_any_byte_says_early_eof() {
        // given: the exit closed the stream without answering
        let fx = setup(&[]);

        // when: the client asks for a stream
        let text = fx.ask_error().await;

        // then: the same error phrase and the new tokio tail (`early eof`)
        assert!(text.contains("reply not received"), "got: {text}");
        assert!(text.contains("early eof"), "got: {text}");
    }

    #[tokio::test]
    async fn version_byte_then_eof_is_exit_busy() {
        // given: the exit sent the version byte only and closed the stream (it has no free place)
        let fx = setup(&[EXIT_PROTOCOL_VERSION]);

        // when: the client asks for a stream
        let err = fx.ask().await.err().expect("an ask error was expected");

        // then: the error is `ExitBusy`, not a missing reply
        assert!(
            err.downcast_ref::<crate::exit::ExitBusy>().is_some(),
            "got: {err:#}"
        );
    }

    #[tokio::test]
    async fn ok_gives_code_and_live_halves() {
        // given: the exit answers `EXIT_CODE_OK`
        let fx = setup(&[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);

        // when: the client asks for a stream
        let res = within(
            "ask",
            ask_exit_to_connect(&fx.client, &fx.pool, pid(2), "example.com", 443),
        )
        .await;

        // then: the reply code is returned together with the stream halves
        let (_send, _recv, code) = res.expect("reply accepted");
        assert_eq!(code, EXIT_CODE_OK);
    }
}

mod request {
    use super::*;

    // assumption: the fields `PeerConnPool.conns: Arc<tokio::sync::Mutex<HashMap<PeerId, Slot>>>` and
    // `Slot = Arc<OnceCell<Arc<dyn PeerConn>>>` have this shape; the plant goes into them.
    #[tokio::test]
    async fn not_sent_says_prior_text() {
        // given: the pool cell holds a connection whose stream opens with a dead write half
        let fx = setup(&[]);
        fx.plant(Rig::DeadSend).await;

        // when: the client asks for a stream
        let text = fx.ask_error().await;

        // then: the text starts with the same write phrase
        assert!(text.starts_with("request not sent"), "got: {text}");
    }

    #[tokio::test]
    async fn failed_send_on_live_connection_is_replaced() {
        // given: the cell holds a live-looking connection whose streams can't be written to; a healthy exit
        let fx = setup(&[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        fx.plant(Rig::DeadSend).await;

        // when: two requests in a row
        let first = fx.ask_error().await;
        let second = fx.ask().await;

        // then: the first fails on the write, the second goes over a new connection and is answered
        assert!(first.starts_with("request not sent"), "got: {first}");
        assert_eq!(second.expect("second request answered").2, EXIT_CODE_OK);
        assert!(
            !fx.slot_holds_planted().await,
            "the broken connection stays in the cell"
        );
    }
}

mod open_failure {
    use super::*;

    #[tokio::test]
    async fn failed_open_on_live_connection_is_replaced() {
        // given: the cell holds a connection that reports itself open but fails every `open`
        let fx = setup(&[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        fx.plant(Rig::FailOpen).await;

        // when: two requests in a row
        let first = fx.ask_error().await;
        let second = fx.ask().await;

        // then: the first says the stream didn't open, the second is answered over a new connection
        assert!(first.contains("didn't open"), "got: {first}");
        assert_eq!(second.expect("second request answered").2, EXIT_CODE_OK);
        assert!(
            !fx.slot_holds_planted().await,
            "the broken connection stays in the cell"
        );
    }
}

mod kept {
    use super::*;

    #[tokio::test]
    async fn early_eof_keeps_connection() {
        // given: the exit closes the stream without a reply, the connection itself is healthy
        let fx = setup(&[]);
        fx.plant(Rig::Pass).await;

        // when: the client asks
        let text = fx.ask_error().await;

        // then: the read fails, the connection stays in the cell
        assert!(text.contains("reply not received"), "got: {text}");
        assert!(fx.slot_holds_planted().await, "the connection was dropped");
    }

    #[tokio::test]
    async fn busy_reply_keeps_connection() {
        // given: the exit answers with the version byte only over a healthy connection
        let fx = setup(&[EXIT_PROTOCOL_VERSION]);
        fx.plant(Rig::Pass).await;

        // when: the client asks
        let err = fx.ask().await.err().expect("an ask error was expected");

        // then: the exit is busy, and the connection stays in the cell
        assert!(
            err.downcast_ref::<crate::exit::ExitBusy>().is_some(),
            "got: {err:#}"
        );
        assert!(fx.slot_holds_planted().await, "the connection was dropped");
    }

    #[tokio::test]
    async fn unsupported_reply_version_keeps_connection() {
        // given: the exit answers with a foreign version over a healthy connection
        let fx = setup(&[9, 0]);
        fx.plant(Rig::Pass).await;

        // when: the client asks
        let text = fx.ask_error().await;

        // then: the version error, the connection stays in the cell
        assert!(
            text.contains("unsupported exit reply version"),
            "got: {text}"
        );
        assert!(fx.slot_holds_planted().await, "the connection was dropped");
    }
}

mod busy {
    use super::*;

    #[tokio::test]
    async fn stuck_open_gives_stream_busy() {
        // given: the connection in the cell opens no streams, the open deadline is short
        let mut fx = setup(&[]);
        fx.pool = fx
            .pool
            .clone()
            .with_open_timeout(Duration::from_millis(100));
        fx.plant(Rig::Stuck).await;

        // when: the pool is asked for a stream
        let res = within("stream", fx.pool.open_peer_stream(&fx.client, pid(2))).await;

        // then: the error is `StreamBusy`
        let err = res.err().expect("stream did not open");
        assert!(err.downcast_ref::<StreamBusy>().is_some(), "got: {err:#}");
    }

    #[tokio::test]
    async fn busy_connection_is_kept() {
        // given: the connection in the cell opens no streams, the open deadline is short
        let mut fx = setup(&[]);
        fx.pool = fx
            .pool
            .clone()
            .with_open_timeout(Duration::from_millis(100));
        fx.plant(Rig::Stuck).await;

        // when: the pool is asked for a stream
        let res = within("stream", fx.pool.open_peer_stream(&fx.client, pid(2))).await;

        // then: busy, and the connection stays in the cell
        assert!(res.is_err(), "stream did not open");
        assert!(fx.slot_holds_planted().await, "the connection was dropped");
    }
}

/// Nodes: client (1) and exit (2, scripted handler), and the client's pool.
struct Fx {
    client: NetStack,
    _server: NetStack,
    pool: PeerConnPool,
    /// `stable_id` of the connection put in by `plant`.
    planted: AtomicU64,
}

impl Fx {
    /// One `ask_exit_to_connect` to the exit.
    async fn ask(&self) -> Result<(BoxSend, BoxRecv, u8)> {
        within(
            "ask",
            ask_exit_to_connect(&self.client, &self.pool, pid(2), "example.com", 443),
        )
        .await
    }

    /// Whether the exit's cell holds the connection put in by `plant`.
    async fn slot_holds_planted(&self) -> bool {
        let map = self.pool.conns.lock().await;
        let id = self.planted.load(Ordering::SeqCst);
        map.get(&pid(2))
            .and_then(|slot| slot.get())
            .is_some_and(|conn| conn.stable_id() == id)
    }

    /// Returns the error text of `ask_exit_to_connect`; success panics.
    async fn ask_error(&self) -> String {
        match within(
            "ask",
            ask_exit_to_connect(&self.client, &self.pool, pid(2), "example.com", 443),
        )
        .await
        {
            Ok(_) => panic!("an ask error was expected"),
            Err(e) => format!("{e:#}"),
        }
    }

    /// Puts into the exit's cell a real connection wrapped in a trick.
    async fn plant(&self, rig: Rig) {
        let conn = within("connect", self.client.connect(pid(2), EXIT))
            .await
            .expect("connection to the exit");
        self.planted.store(conn.stable_id(), Ordering::SeqCst);
        let rigged: Arc<dyn PeerConn> = Arc::new(Rigged { inner: conn, rig });
        let slot: Slot = Arc::new(tokio::sync::OnceCell::new_with(Some(rigged)));
        self.pool.conns.lock().await.insert(pid(2), slot);
    }
}

fn setup(reply: &'static [u8]) -> Fx {
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, Arc::new(Scripted::new(reply)))
        .build()
        .expect("exit built");
    let client = NetStackBuilder::new(secret(1))
        .transport(Spy::on(&net, &secret(1)).shared())
        .build()
        .expect("client built");
    client.add_hints(server.invite_addrs()[0].clone());
    Fx {
        client,
        _server: server,
        pool: PeerConnPool::default(),
        planted: AtomicU64::new(0),
    }
}

#[derive(Clone, Copy)]
enum Rig {
    /// `open` returns a stream that can't be written to (the peer dropped the reading half).
    DeadSend,
    /// `open` never returns.
    Stuck,
    /// `open` fails while `is_closed()` stays `false`.
    FailOpen,
    /// `open` goes to the real connection.
    Pass,
}

/// A wrapper connection: delegates everything to the real one except `open`.
struct Rigged {
    inner: Arc<dyn PeerConn>,
    rig: Rig,
}

impl PeerConn for Rigged {
    fn peer(&self) -> PeerId {
        self.inner.peer()
    }

    fn stable_id(&self) -> u64 {
        self.inner.stable_id()
    }

    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        match self.rig {
            Rig::Stuck => Box::pin(std::future::pending()),
            Rig::FailOpen => Box::pin(async { Err(NetError::Closed) }),
            Rig::Pass => self.inner.open(),
            Rig::DeadSend => Box::pin(async { Ok(dead_send_pair()) }),
        }
    }

    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.accept()
    }

    fn link(&self) -> LinkInfo {
        self.inner.link()
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.inner.close(code, reason)
    }

    fn closed(&self) -> BoxFuture<'_, ()> {
        self.inner.closed()
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

/// A pair whose write fails at once (`BrokenPipe`) and whose read is empty.
fn dead_send_pair() -> StreamPair {
    let (send, gone) = duplex(8);
    drop(gone);
    StreamPair {
        send: Box::new(send),
        recv: Box::new(tokio::io::empty()),
    }
}
