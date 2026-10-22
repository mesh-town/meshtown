//! Tests of the running facade, continued: closing (order, repeat, after close) and the snapshot.

use super::*;
use crate::net::link::NetInfo;
use crate::net::transport::Transport;

mod close {
    use super::*;

    #[tokio::test]
    async fn close_order() {
        // given: node B with a live connection in a handler; discovery, handler and transport write to the log
        let net = MemNet::new();
        let journal = Journal::new();
        let source: Arc<dyn Discovery> = Script::new("mainline").journaled(&journal).shared();
        let opts = Opts {
            probe: Probe::journaled(&journal),
            sources: vec![source],
            journal: Some(journal.clone()),
            ..serving(&[AUTH])
        };
        let (a, b) = (node(&net, 1, Opts::default()), node(&net, 2, opts));
        a.stack.add_hints(b.invite());
        let _live = a.connect(2, AUTH).await.expect("connect succeeded");
        b.probe.next().await;

        // when: B is closed
        within("close", b.stack.close()).await;

        // then: discovery is stopped first, then the handlers, then the transport
        assert_eq!(journal.entries(), vec!["discovery", "handler", "transport"]);
    }

    #[tokio::test]
    async fn close_leaves_the_transport_without_incoming() {
        let net = MemNet::new();
        let b = node(&net, 2, serving(&[AUTH]));

        within("close", b.stack.close()).await;

        let next = within("accept after close", b.spies[0].accept()).await;
        assert!(next.is_none(), "accept of a closed transport is None");
    }

    #[tokio::test]
    async fn connect_after_close_is_closed() {
        let net = MemNet::new();
        let (a, b) = (
            node(&net, 1, Opts::default()),
            node(&net, 2, serving(&[AUTH])),
        );
        a.stack.add_hints(b.invite());
        within("close", a.stack.close()).await;

        let res = a.connect(2, AUTH).await;

        assert!(matches!(res, Err(NetError::Closed)), "expected Closed");
    }

    #[tokio::test]
    async fn publish_after_close_is_closed() {
        let net = MemNet::new();
        let a = node(&net, 1, discovering(&net, &[]));
        within("close", a.stack.close()).await;

        let res = within("publish", a.stack.publish()).await;

        assert!(
            matches!(res, Err(NetError::Closed)),
            "expected Closed: {res:?}"
        );
    }

    #[tokio::test]
    async fn clone_shares_the_closed_state() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());
        let copy = a.stack.clone();

        within("close of the copy", copy.close()).await;

        let res = a.connect(2, AUTH).await;
        assert!(matches!(res, Err(NetError::Closed)), "the original is closed too");
    }

    #[tokio::test]
    async fn close_twice_concurrent() {
        let net = MemNet::new();
        let b = node(&net, 2, serving(&[AUTH]));

        tokio::join!(
            within("first close", b.stack.close()),
            within("second close", b.stack.close())
        );

        assert_eq!(b.spies[0].closes(), 1, "transport close — once");
    }

    #[tokio::test]
    async fn close_again_after_close_returns() {
        let net = MemNet::new();
        let b = node(&net, 2, serving(&[AUTH]));
        within("first close", b.stack.close()).await;

        within("repeated close", b.stack.close()).await;

        assert_eq!(b.spies[0].closes(), 1);
    }

    #[tokio::test]
    async fn stack_is_closed_after_close() {
        // given: a live stack and its clone taken before closing
        let net = MemNet::new();
        let a = node(&net, 1, serving(&[AUTH]));
        let copy = a.stack.clone();
        assert!(!a.stack.is_closed(), "a fresh stack is considered closed");

        // when: the stack is closed
        within("close", a.stack.close()).await;

        // then: is_closed() == true, and for the clone too
        assert!(a.stack.is_closed(), "the stack is not closed after close()");
        assert!(copy.is_closed(), "the clone does not see the close");
    }

    #[tokio::test]
    async fn close_stops_every_transport() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let a = setup(&[&x, &y], 1, Opts::default());

        within("close", a.stack.close()).await;

        assert!(
            a.spies.iter().all(|spy| spy.closes() == 1),
            "both transports are closed"
        );
    }
}

mod snapshot {
    use super::*;

    #[tokio::test]
    async fn snapshot_fields() {
        // given: two transports of different kinds, two sources with priority, three foreign hints (one about itself)
        let (x, y) = (MemNet::new(), MemNet::new());
        let relaying = NetInfo {
            can_relay: true,
            ..NetInfo::default()
        };
        let first = Spy::on(&x, &secret(1))
            .of_kind("alpha")
            .with_info(relaying)
            .shared();
        let second = Spy::on(&y, &secret(1)).of_kind("beta").shared();
        let stack = NetStackBuilder::new(secret(1))
            .transport(first)
            .transport(second)
            .discovery(Script::new("mainline").shared())
            .discovery(Script::new("invite").shared())
            .discovery_priority(vec!["invite", "mainline"])
            .build()
            .expect("build succeeded");
        for seed in [2, 3, 1] {
            stack.add_hints(addr(seed, &[1]));
        }

        // when
        let snap = stack.snapshot();

        // then
        assert_eq!(snap.me, pid(1));
        assert_eq!(
            snap.transport_names,
            vec!["alpha", "beta"],
            "order of transports"
        );
        assert_eq!(
            snap.discovery_names,
            vec!["invite", "mainline"],
            "order of discovery — by priority"
        );
        assert_eq!(snap.net_infos.len(), 2);
        assert!(
            snap.net_infos[0].can_relay && !snap.net_infos[1].can_relay,
            "net_info in the order of transports"
        );
        assert_eq!(snap.known_peer_count, 2, "we do not count ourselves");
    }

    #[tokio::test]
    async fn empty_snapshot_of_a_fresh_node() {
        let net = MemNet::new();
        let a = node(&net, 1, Opts::default());

        let snap = a.stack.snapshot();

        assert_eq!(snap.known_peer_count, 0);
        assert!(snap.discovery_names.is_empty());
        assert_eq!(snap.transport_names, vec!["mem"]);
    }
}
