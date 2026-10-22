use super::*;
use crate::network_key::NetworkKey;

fn cfg_with(names: &[&str], test_net: bool) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.settings.net_choice.transports = names.iter().map(|s| s.to_string()).collect();
    if !test_net {
        cfg.test_net = None;
    }
    cfg
}

fn secret() -> NodeSecret {
    crate::config_file::new_secret().unwrap()
}

/// Names are checked before bring-up: `hyper` without test_net passes the check.
#[test]
fn hyper_only_ok() {
    let names = vec!["hyper".to_string()];
    assert!(check_transport_names(&names, false).is_ok());
}

#[tokio::test]
async fn mem_without_test_net_refused() {
    let err = bind_transports(&cfg_with(&["mem"], false), &secret()).await.err().expect("expected an error");
    assert!(err.to_string().contains("for tests only"), "{err}");
}

#[tokio::test]
async fn unknown_name_refused() {
    let err = bind_transports(&cfg_with(&["iroh"], true), &secret()).await.err().expect("expected an error");
    assert!(err.to_string().contains("is not built; available: hyper"), "{err}");
}

#[tokio::test]
async fn empty_list_refused() {
    let err = bind_transports(&cfg_with(&[], true), &secret()).await.err().expect("expected an error");
    assert!(err.to_string().contains("the transport list is empty"), "{err}");
}

#[tokio::test]
async fn mem_with_test_net_binds() {
    let list = bind_transports(&cfg_with(&["mem"], true), &secret()).await.unwrap().list;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind(), "mem");
}

#[tokio::test]
async fn ephemeral_mem() {
    let net = MemNet::new();
    let t = bind_ephemeral(Some(&net)).await.unwrap();
    assert_eq!(t.kind(), "mem");
    t.close().await;
}
