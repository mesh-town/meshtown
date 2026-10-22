//! The `localnet` mode of the proxy listener: the bind address and admission of a loopback client.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

#[tokio::test]
async fn localnet_binds_all_interfaces() {
    let l = bind_proxy_listener(0, ListenAccess::Localnet)
        .await
        .unwrap();
    assert_eq!(
        l.local_addr().unwrap().ip(),
        std::net::IpAddr::from(std::net::Ipv4Addr::UNSPECIFIED)
    );
}

#[tokio::test]
async fn localhost_binds_loopback_only() {
    let l = bind_proxy_listener(0, ListenAccess::Localhost)
        .await
        .unwrap();
    assert_eq!(
        l.local_addr().unwrap().ip(),
        std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST)
    );
}

/// A node in `localnet` mode: the address in `socks_addr` is loopback, a client from `127.0.0.1` is served.
#[tokio::test]
async fn localnet_serves_loopback_client() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let mut cfg = crate::node::NodeConfig::for_loopback_test(key);
    cfg.socks.access = ListenAccess::Localnet;
    let node = crate::node::start_node(cfg).await.unwrap();
    let addr = node.socks_addr.expect("the proxy is up");
    assert_eq!(
        addr.ip(),
        std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST)
    );

    let mut c = TcpStream::connect(addr).await.unwrap();
    c.write_all(&[5, 1, 0]).await.unwrap();
    let mut reply = [0u8; 2];
    c.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply, [5, 0], "the SOCKS5 greeting was accepted");
    drop(c);
    node.shutdown().await;
}

/// `Only` with a nonexistent name: a loopback client is served anyway (loopback is always admitted).
#[tokio::test]
async fn only_missing_iface_still_serves_loopback_client() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let mut cfg = crate::node::NodeConfig::for_loopback_test(key);
    cfg.socks.access = ListenAccess::Localnet;
    cfg.socks.ifaces = crate::config::config_file::IfaceFilter::Only(vec!["no-such0".to_string()]);
    let node = crate::node::start_node(cfg).await.unwrap();
    let mut c = TcpStream::connect(node.socks_addr.expect("the proxy is up"))
        .await
        .unwrap();
    c.write_all(&[5, 1, 0]).await.unwrap();
    let mut reply = [0u8; 2];
    c.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply, [5, 0]);
    drop(c);
    node.shutdown().await;
}
