//! Tests of `NetStack::proven_addr`: the address a transport itself dialed successfully, as a record for invites.

use super::NetStack;
use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::facade::test_support::{pid, secret, Spy};
use crate::transport::net::facade::NetStackBuilder;
use crate::transport::net::mem::MemNet;
use std::net::SocketAddr;
use std::sync::Arc;

mod transport_that_proved_an_address {
    use super::*;

    #[tokio::test]
    async fn gives_a_record_with_that_address() {
        // given: the only transport proved `203.0.113.7:4000` for node 2
        let net = MemNet::new();
        let stack = stack_over(vec![Spy::on(&net, &secret(1)).proving(pid(2), at())]);
        // when
        let got = stack.proven_addr(pid(2));
        // then
        let want = PeerAddr {
            id: pid(2),
            hints: vec![AddrHint::Ip(at())],
        };
        assert_eq!(got, Some(want));
    }

    #[tokio::test]
    async fn gives_nothing_for_another_node() {
        // given: the proof is about node 2
        let net = MemNet::new();
        let stack = stack_over(vec![Spy::on(&net, &secret(1)).proving(pid(2), at())]);
        // when
        let got = stack.proven_addr(pid(3));
        // then
        assert_eq!(got, None, "node 3 was never dialed");
    }

    #[tokio::test]
    async fn is_found_behind_a_transport_that_proved_nothing() {
        // given: the first transport knows nothing, the second one proved the address
        let net = MemNet::new();
        let silent = Spy::on(&net, &secret(1));
        let proving = Spy::on(&net, &secret(1)).proving(pid(2), at());
        let stack = stack_over(vec![silent, proving]);
        // when
        let got = stack.proven_addr(pid(2));
        // then
        assert_eq!(got.map(|a| a.hints), Some(vec![AddrHint::Ip(at())]));
    }
}

mod transport_without_a_dial_registry {
    use super::*;

    #[tokio::test]
    async fn gives_nothing() {
        // given: a plain `mem` transport, which keeps no proven addresses
        let net = MemNet::new();
        let stack = stack_over(vec![Spy::on(&net, &secret(1))]);
        // when
        let got = stack.proven_addr(pid(2));
        // then
        assert_eq!(got, None, "the default of the trait is no address");
    }

    #[tokio::test]
    async fn gives_nothing_without_the_spy_wrapper_too() {
        // given: the `mem` transport of the network itself
        let net = MemNet::new();
        let stack = NetStackBuilder::new(secret(1))
            .transport(net.transport(&secret(1)))
            .build()
            .expect("a stack over `mem` is built");
        // when
        let got = stack.proven_addr(pid(2));
        // then
        assert_eq!(got, None);
    }
}

/// The documentation address a spy proves.
fn at() -> SocketAddr {
    "203.0.113.7:4000".parse().unwrap()
}

/// A stack of node 1 over the given transports, in order.
fn stack_over(spies: Vec<Spy>) -> NetStack {
    let mut builder = NetStackBuilder::new(secret(1));
    for spy in spies {
        builder = builder.transport(Arc::new(spy));
    }
    builder.build().expect("a stack over spies is built")
}
