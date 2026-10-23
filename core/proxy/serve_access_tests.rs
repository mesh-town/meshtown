//! The `localnet` mode of the proxy listener: the bind address and admission of a loopback client.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;
use crate::config::config_file::IfaceFilter;

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

/// A front in `localnet` mode: the published address is loopback, a client from `127.0.0.1` is served.
#[tokio::test]
async fn localnet_serves_loopback_client() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let front = bound_front(&node, ListenAccess::Localnet, IfaceFilter::default()).await;
    let addr = front.addr().expect("the proxy is up");
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
    front.close().await;
    node.shutdown().await;
}

/// `Only` with a nonexistent name: a loopback client is served anyway (loopback is always admitted).
#[tokio::test]
async fn only_missing_iface_still_serves_loopback_client() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let filter = IfaceFilter::Only(vec!["no-such0".to_string()]);
    let front = bound_front(&node, ListenAccess::Localnet, filter).await;
    let mut c = TcpStream::connect(front.addr().expect("the proxy is up"))
        .await
        .unwrap();
    c.write_all(&[5, 1, 0]).await.unwrap();
    let mut reply = [0u8; 2];
    c.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply, [5, 0]);
    drop(c);
    front.close().await;
    node.shutdown().await;
}

/// A front on a free port with the given access, serving through `node`.
async fn bound_front(
    node: &crate::node::RunningNode,
    access: ListenAccess,
    ifaces: IfaceFilter,
) -> crate::proxy::ProxyFront {
    let settings = crate::config::config_file::ProcessSettings {
        socks_enabled: true,
        socks_port: 0,
        socks_access: access,
        socks_ifaces: ifaces,
        ..Default::default()
    };
    let front = crate::proxy::ProxyFront::bind(&settings).await.unwrap();
    front.set_target(Some(node.proxy_config()));
    front
}
