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
    BoxFuture, LinkInfo, NetResult, NetStack, NetStackBuilder, PeerConn, StreamPair,
};
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
    async fn closed_after_one_byte_says_early_eof() {
        // given: the exit sent one byte of a two-byte reply and closed the stream
        let fx = setup(&[EXIT_PROTOCOL_VERSION]);

        // when: the client asks for a stream
        let text = fx.ask_error().await;

        // then: the same text
        assert!(text.contains("reply not received"), "got: {text}");
        assert!(text.contains("early eof"), "got: {text}");
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
}

/// Nodes: client (1) and exit (2, scripted handler), and the client's pool.
struct Fx {
    client: NetStack,
    _server: NetStack,
    pool: PeerConnPool,
}

impl Fx {
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
    }
}

#[derive(Clone, Copy)]
enum Rig {
    /// `open` returns a stream that can't be written to (the peer dropped the reading half).
    DeadSend,
    /// `open` never returns.
    Stuck,
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
