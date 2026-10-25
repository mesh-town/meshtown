//! Tests of the cell-filling deadline (`pool.rs`): a `connect` that never finishes must not hold the requests
//! queued behind it for longer than the pool's `connect_wait`. The client transport is a wrapper whose
//! `connect` hangs while a flag is set.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::*;
use crate::exit::test_support::Scripted;
use crate::exit::{EXIT_CODE_OK, EXIT_PROTOCOL_VERSION};
use crate::transport::net::facade::test_support::{pid, secret, within, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::EXIT;
use crate::transport::net::{
    BoxFuture, NetInfo, NetResult, NetStackBuilder, PeerAddr, RelayRef, Transport,
};

const WAIT: Duration = Duration::from_millis(150);

#[tokio::test]
async fn hanging_connect_fails_each_waiter_within_the_limit() {
    // given: the client's `connect` hangs, two requests go to the same exit at once
    let fx = setup(true);
    let started = Instant::now();

    // when: both wait for the cell
    let (a, b) = within("waiters", async {
        tokio::join!(
            fx.pool.open_peer_stream(&fx.client, pid(2)),
            fx.pool.open_peer_stream(&fx.client, pid(2))
        )
    })
    .await;

    // then: both fail with the connect text, not later than about twice the limit (plus slack)
    for res in [a, b] {
        let text = format!("{:#}", res.err().expect("the request failed"));
        assert!(text.contains("didn't open"), "got: {text}");
    }
    assert!(started.elapsed() < WAIT * 2 + Duration::from_millis(1500));
}

#[tokio::test]
async fn request_after_wait_limit_connects_again() {
    // given: a request failed because `connect` hung
    let fx = setup(true);
    let first = within("first", fx.pool.open_peer_stream(&fx.client, pid(2))).await;
    assert!(first.is_err(), "the hung connect gave an error");

    // when: the transport recovers and the request is repeated
    fx.hang.store(false, Ordering::SeqCst);
    let second = within("second", fx.pool.open_peer_stream(&fx.client, pid(2))).await;

    // then: the stream opens
    assert!(second.is_ok(), "the cell was not poisoned by the hang");
}

struct Fx {
    client: NetStack,
    _server: NetStack,
    pool: PeerConnPool,
    hang: Arc<AtomicBool>,
}

fn setup(hang: bool) -> Fx {
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(
            EXIT,
            Arc::new(Scripted::new(&[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK])),
        )
        .build()
        .expect("exit built");
    let hang = Arc::new(AtomicBool::new(hang));
    let transport = Arc::new(Hanging {
        inner: Spy::on(&net, &secret(1)).shared(),
        hang: hang.clone(),
    });
    let client = NetStackBuilder::new(secret(1))
        .transport(transport)
        .build()
        .expect("client built");
    client.add_hints(server.invite_addrs()[0].clone());
    Fx {
        client,
        _server: server,
        pool: PeerConnPool::default().with_connect_wait(WAIT),
        hang,
    }
}

/// A wrapper transport: `connect` never finishes while `hang` is set; the rest is delegated.
struct Hanging {
    inner: Arc<dyn Transport>,
    hang: Arc<AtomicBool>,
}

impl Transport for Hanging {
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }
    fn me(&self) -> PeerId {
        self.inner.me()
    }
    fn local_addr(&self) -> PeerAddr {
        self.inner.local_addr()
    }
    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.inner.can_reach(to)
    }
    fn add_hints(&self, addr: &PeerAddr) {
        self.inner.add_hints(addr)
    }
    fn set_relays(&self, relays: &[RelayRef]) {
        self.inner.set_relays(relays)
    }
    fn serve(&self, svcs: &[ServiceId]) {
        self.inner.serve(svcs)
    }
    fn connect(
        &self,
        to: &PeerAddr,
        svc: ServiceId,
    ) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        let hang = self.hang.load(Ordering::SeqCst);
        let fut = self.inner.connect(to, svc);
        Box::pin(async move {
            if hang {
                std::future::pending::<()>().await;
            }
            fut.await
        })
    }
    fn accept(&self) -> BoxFuture<'_, Option<(ServiceId, Arc<dyn PeerConn>)>> {
        self.inner.accept()
    }
    fn net_info(&self) -> NetInfo {
        self.inner.net_info()
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        self.inner.close()
    }
}
