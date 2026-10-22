//! Tests of the running facade (`stack.rs`): route choice, handlers, hints and addresses.
//! Closing and the snapshot are in `stack_close_tests.rs`.

use super::NetStack;
use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::discovery::Discovery;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::facade::test_support::{
    addr, pid, secret, within, Journal, Probe, Script, ScriptReply, Spy,
};
use crate::transport::net::facade::NetStackBuilder;
use crate::transport::net::id::PeerId;
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::{ServiceId, AUTH, EXIT};
use crate::transport::net::transport::PeerConn;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

mod routing {
    use super::*;

    #[tokio::test]
    async fn connect_routes_to_handler() {
        let net = MemNet::new();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(&net, 2, serving(&[AUTH])),
        );
        a.stack.add_hints(b.invite());

        let conn = a.connect(2, AUTH).await.expect("connect succeeded");

        let at_b = b.probe.next().await;
        assert_eq!(at_b.peer(), pid(1), "the handler of B sees counterpart A");
        assert_eq!(conn.peer(), pid(2), "A sees B");
    }

    #[tokio::test]
    async fn connect_via_discovery() {
        let net = MemNet::new();
        let a = node(&net, 1, discovering(&net, &[]));
        let b = node(&net, 2, discovering(&net, &[AUTH]));
        within("publish", b.stack.publish())
            .await
            .expect("publish succeeded");

        let conn = within("connect with retries", connect_until_ok(&a.stack, 2, AUTH)).await;

        assert_eq!(conn.peer(), pid(2));
        assert_eq!(b.probe.next().await.peer(), pid(1));
    }

    #[tokio::test]
    async fn no_route_fast() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());

        let res = tokio::time::timeout(Duration::from_millis(300), a.connect(9, AUTH)).await;

        let res = res.expect("connect to an unknown node must not wait");
        assert!(matches!(res, Err(NetError::NoRoute)), "expected NoRoute");
    }

    #[tokio::test]
    async fn self_no_route() {
        let net = MemNet::new();
        let a = node(&net, 1, serving(&[AUTH]));

        let res = a.connect(1, AUTH).await;

        assert!(matches!(res, Err(NetError::NoRoute)), "to itself — NoRoute");
    }

    #[tokio::test]
    async fn first_reaching_transport() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let a = setup(&[&x, &y], 1, Opts::default());
        let b = node(&y, 2, serving(&[AUTH]));
        a.stack.add_hints(b.invite());

        let conn = a.connect(2, AUTH).await.expect("via the second transport");

        assert_eq!(conn.peer(), pid(2));
        assert_eq!(a.spies[0].connects().len(), 0, "X does not reach B");
        assert_eq!(a.spies[1].connects(), vec![(pid(2), AUTH)]);
        assert_eq!(b.probe.next().await.peer(), pid(1));
    }

    #[tokio::test]
    async fn first_of_several_reaching_wins() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let a = setup(&[&x, &y], 1, Opts::default());
        let b = setup(&[&x, &y], 2, serving(&[AUTH]));
        b.stack
            .invite_addrs()
            .into_iter()
            .for_each(|one| a.stack.add_hints(one));

        a.connect(2, AUTH).await.expect("connect succeeded");

        assert_eq!(
            a.spies[0].connects().len(),
            1,
            "the first one that reaches is X"
        );
        assert_eq!(a.spies[1].connects().len(), 0, "the second is not needed");
    }

    #[tokio::test]
    async fn first_transport_error_not_retried_on_next() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let a = setup(&[&x, &y], 1, Opts::default());
        let b = setup(&[&x, &y], 2, serving(&[AUTH]));
        b.stack
            .invite_addrs()
            .into_iter()
            .for_each(|one| a.stack.add_hints(one));
        x.cut(pid(1), pid(2));

        let res = a.connect(2, AUTH).await;

        assert!(res.is_err(), "a broken first path is an error going up");
        assert_eq!(a.spies[0].connects().len(), 1);
        assert_eq!(
            a.spies[1].connects().len(),
            0,
            "the next transport is not tried"
        );
    }

    #[tokio::test]
    async fn connect_hands_hints_to_the_chosen_transport() {
        let net = MemNet::new();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(&net, 2, serving(&[AUTH])),
        );
        a.stack.add_hints(b.invite());

        a.connect(2, AUTH).await.expect("connect succeeded");

        let given = a.spies[0].hints();
        assert!(
            given.iter().any(|h| h.id == pid(2)),
            "the transport received the address of B: {given:?}"
        );
    }
}

mod handlers {
    use super::*;

    #[tokio::test]
    async fn unserved_never_handled() {
        let net = MemNet::new();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(&net, 2, serving(&[AUTH])),
        );
        a.stack.add_hints(b.invite());

        let res = a.connect(2, EXIT).await;

        if let Ok(conn) = res {
            within("closed at A", conn.closed()).await;
        }
        assert_eq!(b.probe.calls(), 0, "the AUTH handler must not receive EXIT");
        assert!(b.probe.try_next().is_none());
    }

    #[tokio::test]
    async fn unserved_closed_by_facade_when_transport_breaks_serve() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());
        let b = node(
            &net,
            2,
            Opts {
                serve_all: true,
                ..serving(&[AUTH])
            },
        );
        a.stack.add_hints(b.invite());

        // the spy of B holds the accepted connection: dropped, it would have closed with code 0 even without the facade
        let conn = a
            .connect(2, EXIT)
            .await
            .expect("transport of B accepted EXIT");

        within("closed at A: facade of B closed it", conn.closed()).await;
        assert_eq!(b.probe.calls(), 0, "it did not reach the handler");
    }

    #[tokio::test]
    async fn slow_handler_not_blocking() {
        let net = MemNet::new();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(&net, 2, serving(&[AUTH])),
        );
        a.stack.add_hints(b.invite());

        let mut kept = Vec::new();
        for _ in 0..3 {
            kept.push(a.connect(2, AUTH).await.expect("connect succeeded"));
        }

        for _ in 0..3 {
            b.probe.next().await;
        }
        assert_eq!(
            b.probe.calls(),
            3,
            "the eternal handler does not hold up the acceptor"
        );
    }

    #[tokio::test]
    async fn handler_panic_isolated() {
        let net = MemNet::new();
        let probe = Probe::panicking_first();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(
                &net,
                2,
                Opts {
                    probe,
                    ..serving(&[AUTH])
                },
            ),
        );
        a.stack.add_hints(b.invite());
        a.connect(2, AUTH)
            .await
            .expect("the first connect succeeded");
        sleep(Duration::from_millis(100)).await; // the handler has already crashed

        let second = a
            .connect(2, AUTH)
            .await
            .expect("the acceptor is alive after the panic");

        assert_eq!(
            b.probe.next().await.peer(),
            pid(1),
            "the second one reached the handler"
        );
        assert_eq!(second.peer(), pid(2));
        within("close after the panic", b.stack.close()).await;
    }
}

mod weak {
    use super::*;

    /// given: a live stack → when: the weak handle is upgraded → then: `Some` with the same `me`.
    #[tokio::test]
    async fn upgrades_while_alive() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());
        let weak = a.stack.downgrade();

        let up = weak.upgrade().expect("the stack is alive");

        assert_eq!(up.me(), pid(1));
    }

    /// given: a weak handle → when: the last `NetStack` is dropped → then: `upgrade` gives `None`.
    #[tokio::test]
    async fn none_after_drop() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());
        let weak = a.stack.downgrade();

        drop(a);

        assert!(
            weak.upgrade().is_none(),
            "the handle does not hold the stack"
        );
    }
}

#[path = "stack_hints_tests.rs"]
mod hints_and_addresses;

#[path = "stack_close_tests.rs"]
mod closing;

#[path = "stack_drop_tests.rs"]
mod dropping;

/// The node under test: a facade, its handler and spies on its transports (one per network).
struct Node {
    stack: NetStack,
    probe: Arc<Probe>,
    spies: Vec<Arc<Spy>>,
}

impl Node {
    /// The first address for an invite.
    fn invite(&self) -> PeerAddr {
        self.stack.invite_addrs()[0].clone()
    }

    /// `connect` to the node with the seed `seed`.
    async fn connect(&self, seed: u8, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        within("connect", self.stack.connect(pid(seed), svc)).await
    }
}

/// Node preconditions: which services it serves, which discovery sources, what it spies with.
struct Opts {
    serves: Vec<ServiceId>,
    probe: Arc<Probe>,
    sources: Vec<Arc<dyn Discovery>>,
    priority: Vec<&'static str>,
    serve_all: bool,
    journal: Option<Journal>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            serves: Vec::new(),
            probe: Probe::new(),
            sources: Vec::new(),
            priority: Vec::new(),
            serve_all: false,
            journal: None,
        }
    }
}

fn serving(svcs: &[ServiceId]) -> Opts {
    Opts {
        serves: svcs.to_vec(),
        ..Opts::default()
    }
}

/// A node with `mem` discovery (the shared board of network `net`, publishing) and services `svcs`.
fn discovering(net: &Arc<MemNet>, svcs: &[ServiceId]) -> Opts {
    let board: Arc<dyn Discovery> = net.discovery("mem", true);
    Opts {
        sources: vec![board],
        ..serving(svcs)
    }
}

fn node(net: &Arc<MemNet>, seed: u8, opts: Opts) -> Node {
    setup(&[net], seed, opts)
}

/// Node `seed` with a transport on each network from `nets`.
fn setup(nets: &[&Arc<MemNet>], seed: u8, opts: Opts) -> Node {
    let spies: Vec<Arc<Spy>> = nets.iter().map(|net| spy_on(net, seed, &opts)).collect();
    let mut builder = NetStackBuilder::new(secret(seed)).discovery_priority(opts.priority.clone());
    for spy in &spies {
        builder = builder.transport(spy.clone());
    }
    for source in &opts.sources {
        builder = builder.discovery(source.clone());
    }
    for svc in &opts.serves {
        builder = builder.serve(*svc, opts.probe.clone());
    }
    let stack = builder
        .build()
        .unwrap_or_else(|e| panic!("node {seed} was not built: {e}"));
    Node {
        stack,
        probe: opts.probe,
        spies,
    }
}

fn spy_on(net: &Arc<MemNet>, seed: u8, opts: &Opts) -> Arc<Spy> {
    let mut spy = Spy::on(net, &secret(seed));
    if opts.serve_all {
        spy = spy.serving_everything();
    }
    if let Some(journal) = &opts.journal {
        spy = spy.journaled(journal);
    }
    spy.shared()
}

/// Repeats `connect` while it returns `NoRoute` (discovery has not brought the address yet); the time limit is on the caller.
async fn connect_until_ok(stack: &NetStack, seed: u8, svc: ServiceId) -> Arc<dyn PeerConn> {
    let id: PeerId = pid(seed);
    loop {
        match stack.connect(id, svc).await {
            Ok(conn) => return conn,
            Err(NetError::NoRoute) => sleep(Duration::from_millis(20)).await,
            Err(other) => panic!("connect returned something other than NoRoute: {other}"),
        }
    }
}

#[path = "stack_addr_tests.rs"]
mod addr_tests;
