//! Tests for `SplitTransport` (`split.rs`) with a stub carrier: accept, connect, close.
//! The connector is a real `MemConnector`, the carrier is a `StubCarrier` that, by script, hangs,
//! refuses or accepts, and records the role it was started with.

use super::*;
use crate::net::mem::{MemConnector, MemNet};
use crate::net::{
    AddrHint, BoxFuture, Carrier, Connector, LinkInfo, NetError, NetInfo, NetResult, NodeSecret,
    PacketPath, PathInfo, PeerAddr, PeerConn, PeerId, CarrierRole, ServiceId, StreamPair, Transport,
};
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{sleep, timeout};

const SVC: ServiceId = ServiceId::new(b"test/svc/1");
const OTHER: ServiceId = ServiceId::new(b"test/svc/2");

mod incoming_paths {
    use super::*;

    #[tokio::test]
    async fn a_start_stuck_forever_does_not_block_the_next_path() {
        let fx = setup(Opts {
            steps: vec![Step::Hang, Step::Accept(SVC)],
            serve: Some(vec![SVC]),
            ..Opts::default()
        });

        let _stuck = fx.when_remote_dials().await;
        let _good = fx.when_remote_dials().await;

        let (svc, conn) = fx.then_accepts_one().await;
        assert_eq!(svc, SVC);
        assert_eq!(conn.peer(), peer(2));
    }

    #[tokio::test]
    async fn a_refused_start_is_skipped_silently() {
        let fx = setup(Opts {
            steps: vec![Step::Refuse, Step::Accept(SVC)],
            serve: Some(vec![SVC]),
            ..Opts::default()
        });

        let _refused = fx.when_remote_dials().await;
        let _good = fx.when_remote_dials().await;

        let (svc, _conn) = fx.then_accepts_one().await;
        assert_eq!(svc, SVC);
    }

    #[tokio::test]
    async fn a_full_accept_queue_holds_its_task_not_the_loop() {
        let fx = setup(Opts {
            serve: Some(vec![SVC]),
            ..Opts::default()
        });

        let mut dialed = Vec::new();
        for _ in 0..70 {
            dialed.push(fx.when_remote_dials().await); // the accept queue is 64, nobody accepts
        }

        fx.then_carrier_started(70).await;
        for _ in 0..70 {
            fx.then_accepts_one().await;
        }
    }

    #[tokio::test]
    async fn dropping_the_transport_closes_the_paths_of_stuck_starts() {
        let fx = setup(Opts {
            steps: vec![Step::Hang],
            serve: Some(vec![SVC]),
            ..Opts::default()
        });
        let dialed = fx.when_remote_dials().await;
        fx.then_carrier_started(1).await;

        drop(fx.transport);

        let got = timeout(Duration::from_secs(2), dialed.recv()).await;
        let err = got
            .expect("recv hung: the accept loop outlived Drop")
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }
}

mod served_services {
    use super::*;

    #[tokio::test]
    async fn before_serve_the_carrier_gets_an_empty_list() {
        let fx = setup(Opts::default());

        let _dialed = fx.when_remote_dials().await;

        fx.then_carrier_started(1).await;
        assert_eq!(fx.carrier.started()[0].accept_list, Some(Vec::new()));
    }

    #[tokio::test]
    async fn the_carrier_gets_the_list_given_to_serve() {
        let fx = setup(Opts {
            serve: Some(vec![SVC, OTHER]),
            ..Opts::default()
        });

        let _dialed = fx.when_remote_dials().await;

        fx.then_carrier_started(1).await;
        let list = fx.carrier.started()[0]
            .accept_list
            .clone()
            .expect("role Accept");
        assert_eq!(list.len(), 2, "list from serve: {list:?}");
        assert!(
            list.contains(&SVC) && list.contains(&OTHER),
            "list from serve: {list:?}"
        );
    }

    #[tokio::test]
    async fn a_second_serve_replaces_the_list() {
        let fx = setup(Opts {
            serve: Some(vec![SVC]),
            ..Opts::default()
        });
        fx.transport.serve(&[OTHER]);

        let _dialed = fx.when_remote_dials().await;

        fx.then_carrier_started(1).await;
        assert_eq!(fx.carrier.started()[0].accept_list, Some(vec![OTHER]));
    }
}

#[path = "split_connect_tests.rs"]
mod connect_and_close;

// ---- carrier stub ----

/// What the next `start` of the stub does.
#[derive(Clone, Copy)]
enum Step {
    /// Hang forever, holding the path.
    Hang,
    /// Close the path and refuse (`Refused`).
    Refuse,
    /// Return a connection for the service.
    Accept(ServiceId),
}

/// Record of one `start`: the role and the peer on the path.
#[derive(Clone, Debug)]
struct Started {
    dialed: Option<ServiceId>,
    accept_list: Option<Vec<ServiceId>>,
    peer: PeerId,
}

struct StubCarrier {
    can_carry: bool,
    script: Mutex<VecDeque<Step>>,
    started: Mutex<Vec<Started>>,
}

impl StubCarrier {
    fn new(can_carry: bool, steps: Vec<Step>) -> Arc<StubCarrier> {
        Arc::new(StubCarrier {
            can_carry,
            script: Mutex::new(steps.into()),
            started: Mutex::new(Vec::new()),
        })
    }

    /// The script is over — accept (`SVC`).
    fn next_step(&self) -> Step {
        let next = self.script.lock().unwrap().pop_front();
        next.unwrap_or(Step::Accept(SVC))
    }

    fn started(&self) -> Vec<Started> {
        self.started.lock().unwrap().clone()
    }

    fn note(&self, path: &dyn PacketPath, role: &CarrierRole) {
        let (dialed, accept_list) = match role {
            CarrierRole::Dial(svc) => (Some(*svc), None),
            CarrierRole::Accept(list) => (None, Some(list.to_vec())),
        };
        let peer = path.info().peer;
        self.started.lock().unwrap().push(Started {
            dialed,
            accept_list,
            peer,
        });
    }
}

impl Carrier for StubCarrier {
    fn kind(&self) -> &'static str {
        "stub"
    }

    fn can_carry(&self, _path: &PathInfo) -> bool {
        self.can_carry
    }

    fn start(
        &self,
        path: Box<dyn PacketPath>,
        role: CarrierRole,
    ) -> BoxFuture<'_, NetResult<(ServiceId, Arc<dyn PeerConn>)>> {
        let step = self.next_step();
        self.note(&*path, &role);
        Box::pin(async move {
            match step {
                Step::Hang => std::future::pending().await,
                Step::Refuse => {
                    path.close();
                    Err(NetError::Refused)
                }
                Step::Accept(svc) => {
                    let conn = StubConn {
                        peer: path.info().peer,
                        _path: path,
                    };
                    Ok((svc, Arc::new(conn) as Arc<dyn PeerConn>))
                }
            }
        })
    }
}

/// Stub connection: holds the path, provides no streams.
struct StubConn {
    peer: PeerId,
    _path: Box<dyn PacketPath>,
}

impl PeerConn for StubConn {
    fn peer(&self) -> PeerId {
        self.peer
    }

    fn stable_id(&self) -> u64 {
        0
    }

    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(async { Err(NetError::Closed) })
    }

    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(async { Err(NetError::Closed) })
    }

    fn link(&self) -> LinkInfo {
        LinkInfo::default()
    }

    fn close(&self, _code: u32, _reason: &[u8]) {}

    fn closed(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
}

// ---- setup ----

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn peer(n: u8) -> PeerId {
    secret(n).peer_id()
}

/// Test preconditions.
struct Opts {
    can_carry: bool,
    steps: Vec<Step>,
    /// `None` — `serve` was not called.
    serve: Option<Vec<ServiceId>>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            can_carry: true,
            steps: Vec::new(),
            serve: None,
        }
    }
}

/// Node 1 is a `SplitTransport` with a `mem` connector and a stub carrier; node 2 is a plain `mem` connector.
struct Fixture {
    transport: SplitTransport,
    connector: Arc<MemConnector>,
    carrier: Arc<StubCarrier>,
    remote: Arc<MemConnector>,
    _net: Arc<MemNet>,
}

fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let connector = net.connector(&secret(1));
    let remote = net.connector(&secret(2));
    let carrier = StubCarrier::new(opts.can_carry, opts.steps);
    let transport = SplitTransport::new(peer(1), connector.clone(), carrier.clone());
    if let Some(svcs) = opts.serve {
        transport.serve(&svcs);
    }
    Fixture {
        transport,
        connector,
        carrier,
        remote,
        _net: net,
    }
}

impl Fixture {
    /// Node 2 dials node 1 — the path goes into the transport's accept.
    async fn when_remote_dials(&self) -> Box<dyn PacketPath> {
        let to = self.connector.local_addr();
        let dialing = self.remote.dial(&to);
        match timeout(Duration::from_secs(3), dialing).await {
            Ok(Ok(path)) => path,
            Ok(Err(e)) => panic!("node 2's dial must succeed, got {e:?}"),
            Err(_) => panic!("node 2's dial hung"),
        }
    }

    /// The transport connects to node 2 for the service `svc`.
    async fn when_connecting(&self, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        let to = self.remote.local_addr();
        let res = timeout(Duration::from_secs(3), self.transport.connect(&to, svc)).await;
        res.expect("connect hung")
    }

    async fn accepted(&self) -> Option<(ServiceId, Arc<dyn PeerConn>)> {
        let res = timeout(Duration::from_secs(3), self.transport.accept()).await;
        res.expect("accept hung")
    }

    async fn then_accepts_one(&self) -> (ServiceId, Arc<dyn PeerConn>) {
        self.accepted()
            .await
            .expect("accept returned None, a connection was awaited")
    }

    /// Node 2 takes the path that came to it from the transport.
    async fn then_remote_listens(&self) -> Box<dyn PacketPath> {
        let res = timeout(Duration::from_secs(3), self.remote.listen()).await;
        res.expect("node 2's listen hung")
            .expect("node 2's listen returned None")
    }

    /// Waits up to 3 s until the carrier has been started `n` times.
    async fn then_carrier_started(&self, n: usize) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while self.carrier.started().len() < n {
            assert!(
                tokio::time::Instant::now() < deadline,
                "carrier started {} times out of {n}",
                self.carrier.started().len()
            );
            sleep(Duration::from_millis(10)).await;
        }
    }
}
