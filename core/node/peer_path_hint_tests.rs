//! Tests of the address hint in `record_peer_up` (`node/peer_path.rs`): which connection `LinkInfo`
//! gives which hint, what the spy transport gets and whether telemetry sees the member online.
//! `link()` is replaced by a wrapper over a real `mem` connection. The neighboring `peer_path_tests.rs` is not edited.

use crate::node::telemetry::Telemetry;
use crate::transport::net::contract::{must_finish, secret, SEED_A, SEED_B};
use crate::transport::net::facade::test_support::Spy;
use crate::transport::net::{
    AddrHint, BoxFuture, LinkInfo, MemNet, NetResult, NetStackBuilder, PathKind, PeerAddr,
    PeerConn, PeerId, RelayRef, SplitTransport, StreamPair, Transport, AUTH,
};
use std::net::SocketAddr;
use std::sync::Arc;

use super::record_peer_up;

/// A connection with the given `link()`; everything else comes from the real connection inside.
struct FakeLink {
    inner: Arc<dyn PeerConn>,
    link: LinkInfo,
}

impl PeerConn for FakeLink {
    fn peer(&self) -> PeerId {
        self.inner.peer()
    }
    fn stable_id(&self) -> u64 {
        self.inner.stable_id()
    }
    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.open()
    }
    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.accept()
    }
    fn link(&self) -> LinkInfo {
        self.link.clone()
    }
    fn close(&self, code: u32, reason: &[u8]) {
        self.inner.close(code, reason)
    }
    fn closed(&self) -> BoxFuture<'_, ()> {
        self.inner.closed()
    }
}

struct Fixture {
    spy: Arc<Spy>,
    far: Arc<SplitTransport>,
    real: Arc<dyn PeerConn>,
    _accepted: Arc<dyn PeerConn>,
}

async fn setup() -> Fixture {
    let net = MemNet::new();
    let spy = Spy::on(&net, &secret(SEED_A)).shared();
    let far = net.transport(&secret(SEED_B));
    far.serve(&[AUTH]);
    let (dialed, accepted) = tokio::join!(
        must_finish(spy.connect(&far.local_addr(), AUTH)),
        must_finish(far.accept())
    );
    let real = dialed.expect("connect mem gave no connection");
    let (_, accepted) = accepted.expect("accept mem closed");
    Fixture {
        spy,
        far,
        real,
        _accepted: accepted,
    }
}

impl Fixture {
    fn conn(&self, link: LinkInfo) -> FakeLink {
        FakeLink {
            inner: self.real.clone(),
            link,
        }
    }

    /// Calls `record_peer_up` and returns the hint; telemetry is checked here too.
    async fn record(&self, conn: &FakeLink) -> Option<AddrHint> {
        let tel = Telemetry::new();
        let stack = NetStackBuilder::new(secret(SEED_A))
            .transport(self.spy.clone())
            .build()
            .unwrap();
        let hint = record_peer_up(conn, &stack, &tel);
        // the hint reaches the transport through the discovery handout — asynchronously
        tokio::time::sleep(SETTLE).await;
        let view = tel.view();
        let live = view
            .peers
            .get(&conn.peer())
            .expect("the member is not recorded in telemetry");
        assert!(live.online, "the member is not marked online");
        hint
    }

    async fn close(self) {
        must_finish(self.spy.close()).await;
        must_finish(self.far.close()).await;
    }
}

/// Time for handing the hint to the transport (and for the absence of a hint to mean something).
const SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

fn sock() -> SocketAddr {
    "203.0.113.7:4455".parse().unwrap()
}

mod direct_path {
    use super::*;

    #[tokio::test]
    async fn with_remote_gives_ip_hint_and_passes_it_to_transport() {
        // given: a direct path with a socket address
        let fx = setup().await;
        let conn = fx.conn(LinkInfo {
            path: PathKind::Direct,
            remote: Some(sock()),
            ..LinkInfo::default()
        });
        // when
        let hint = fx.record(&conn).await;
        // then: the hint is Ip, and exactly it went to the transport for this peer
        assert_eq!(hint, Some(AddrHint::Ip(sock())));
        let want = PeerAddr {
            id: conn.peer(),
            hints: vec![AddrHint::Ip(sock())],
        };
        assert_eq!(fx.spy.hints(), vec![want]);
        fx.close().await;
    }

    #[tokio::test]
    async fn without_remote_gives_no_hint() {
        // given: a direct path without a socket address — this is how hyper answers today (net_hyper/link_tests.rs: remote.is_none())
        let fx = setup().await;
        let conn = fx.conn(LinkInfo {
            path: PathKind::Direct,
            remote: None,
            ..LinkInfo::default()
        });
        // when
        let hint = fx.record(&conn).await;
        // then: no hint, nothing passed to the transport
        assert_eq!(hint, None);
        assert!(
            fx.spy.hints().is_empty(),
            "the transport got: {:?}",
            fx.spy.hints()
        );
        fx.close().await;
    }
}

mod relayed_path {
    use super::*;

    #[tokio::test]
    async fn gives_relay_hint_and_passes_it_to_transport() {
        // given: a path through a relay
        let fx = setup().await;
        let relay = RelayRef::Url("https://relay.example".into());
        let conn = fx.conn(LinkInfo {
            path: PathKind::Relayed(relay.clone()),
            ..LinkInfo::default()
        });
        // when
        let hint = fx.record(&conn).await;
        // then
        assert_eq!(hint, Some(AddrHint::Relay(relay.clone())));
        let want = PeerAddr {
            id: conn.peer(),
            hints: vec![AddrHint::Relay(relay)],
        };
        assert_eq!(fx.spy.hints(), vec![want]);
        fx.close().await;
    }
}
