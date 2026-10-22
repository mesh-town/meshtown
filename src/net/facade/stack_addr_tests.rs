//! Tests of `NetStack::connect_addr` and `can_reach` (dialing a ready address, without discovery).

use super::*;
use crate::net::addr::AddrHint;

#[tokio::test]
async fn connect_addr_mem_hint() {
    let net = MemNet::new();
    let (a, b) = (node(&net, 1, Opts::default()), node(&net, 2, serving(&[AUTH])));
    let to = b.invite();
    assert!(a.stack.can_reach(&to), "an address with a mem hint is reachable");

    let conn = within("connect_addr", a.stack.connect_addr(&to, AUTH)).await.expect("connection");

    assert_eq!(conn.peer(), pid(2));
    assert_eq!(b.probe.next().await.peer(), pid(1));
}

#[tokio::test]
async fn connect_addr_no_transport() {
    let net = MemNet::new();
    let a = node(&net, 1, Opts::default());
    let alien = PeerAddr { id: pid(2), hints: vec![AddrHint::Ip("127.0.0.1:9".parse().unwrap())] };
    assert!(!a.stack.can_reach(&alien), "an Ip hint is not for mem");
    let err = within("connect_addr", a.stack.connect_addr(&alien, AUTH)).await.err().expect("error");
    assert!(matches!(err, NetError::NoRoute), "{err:?}");
}

#[tokio::test]
async fn connect_addr_self_refused() {
    let net = MemNet::new();
    let a = node(&net, 1, Opts::default());
    let err = within("connect_addr", a.stack.connect_addr(&a.invite(), AUTH)).await.err().expect("error");
    assert!(matches!(err, NetError::NoRoute), "{err:?}");
}

#[tokio::test]
async fn connect_addr_after_close() {
    let net = MemNet::new();
    let (a, b) = (node(&net, 1, Opts::default()), node(&net, 2, serving(&[AUTH])));
    a.stack.close().await;
    let err = within("connect_addr", a.stack.connect_addr(&b.invite(), AUTH)).await.err().expect("error");
    assert!(matches!(err, NetError::Closed), "{err:?}");
}

#[tokio::test]
async fn connect_uses_lookup() {
    let net = MemNet::new();
    let a = node(&net, 1, discovering(&net, &[]));
    let b = node(&net, 2, discovering(&net, &[AUTH]));
    within("publish", b.stack.publish()).await.expect("publish");
    let conn = within("connect with retries", connect_until_ok(&a.stack, 2, AUTH)).await;
    assert_eq!(conn.peer(), pid(2));
}
