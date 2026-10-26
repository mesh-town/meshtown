//! Tests of how `ask_exit_to_connect` reads the exit's reply: a lone foreign version byte, every two-byte
//! code, a session that ends right after the version byte. The exit is a scripted handler.

use super::*;
use crate::exit::frame::EXIT_PROTOCOL_VERSION;
use crate::exit::test_support::Scripted;
use crate::exit::ExitBusy;
use crate::transport::net::facade::test_support::{pid, secret, within, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::EXIT;
use crate::transport::net::{BoxFuture, NetStack, NetStackBuilder, PeerConn, ServiceHandler};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Reply codes 0…7 and one the client does not know.
static TWO_BYTE_REPLIES: [[u8; 2]; 9] = [
    [1, 0],
    [1, 1],
    [1, 2],
    [1, 3],
    [1, 4],
    [1, 5],
    [1, 6],
    [1, 7],
    [1, 8],
];

mod one_byte {
    use super::*;

    #[tokio::test]
    async fn foreign_version_byte_alone_is_not_busy() {
        // given: the exit sends a foreign version byte and closes the stream
        let fx = setup(Arc::new(Scripted::new(&[EXIT_PROTOCOL_VERSION + 1])));

        // when: the client asks
        let err = fx.ask().await.err().expect("an ask error was expected");

        // then: the version is unsupported, and it is not "busy"
        assert!(err.downcast_ref::<ExitBusy>().is_none(), "got: {err:#}");
        assert!(
            format!("{err:#}").contains("unsupported exit reply version"),
            "got: {err:#}"
        );
    }

    #[tokio::test]
    async fn version_byte_then_session_abort_is_not_busy() {
        // given: the exit sends the version byte and then ends the whole session
        let fx = setup(Arc::new(CutAfterVersion));

        // when: the client asks
        let err = fx.ask().await.err().expect("an ask error was expected");

        // then: a failed read, not "busy"
        assert!(err.downcast_ref::<ExitBusy>().is_none(), "got: {err:#}");
        assert!(
            format!("{err:#}").contains("reply not received"),
            "got: {err:#}"
        );
    }
}

mod two_bytes {
    use super::*;

    #[tokio::test]
    async fn every_two_byte_reply_returns_its_code() {
        for (code, reply) in TWO_BYTE_REPLIES.iter().enumerate() {
            // given: the exit answers `[1, code]`
            let fx = setup(Arc::new(Scripted::new(reply)));

            // when: the client asks
            let answer = fx.ask().await;

            // then: the code comes back as it is
            let (_send, _recv, got) = answer.expect("a two-byte reply is accepted");
            assert_eq!(usize::from(got), code);
        }
    }
}

// ---- setup and helpers ----

struct Fx {
    client: NetStack,
    _server: NetStack,
    pool: PeerConnPool,
}

impl Fx {
    async fn ask(&self) -> Result<(BoxSend, BoxRecv, u8)> {
        within(
            "ask",
            ask_exit_to_connect(&self.client, &self.pool, pid(2), "example.com", 443),
        )
        .await
    }
}

fn setup(handler: Arc<dyn ServiceHandler>) -> Fx {
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, handler)
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

/// An exit that reads the request, writes the version byte and closes the whole session.
struct CutAfterVersion;

impl ServiceHandler for CutAfterVersion {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            let Ok(mut pair) = conn.accept().await else {
                return;
            };
            let mut head = [0u8; 2];
            let _ = pair.recv.read_exact(&mut head).await;
            let mut tail = vec![0u8; usize::from(head[1]) + 2];
            let _ = pair.recv.read_exact(&mut tail).await;
            let _ = pair.send.write_all(&[EXIT_PROTOCOL_VERSION]).await;
            conn.close(0, b"cut");
            std::future::pending::<()>().await;
        })
    }
}
