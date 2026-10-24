use super::*;
use crate::members::network_key::NetworkKey;

fn cfg_with(names: &[&str], test_net: bool) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.settings.net_choice.transports = names.iter().map(|s| s.to_string()).collect();
    if !test_net {
        cfg.test_net = None;
    }
    cfg
}

fn secret() -> NodeSecret {
    crate::config::config_file::new_secret().unwrap()
}

/// Names are checked before bring-up: `hyper` without test_net passes the check.
#[test]
fn hyper_only_ok() {
    let names = vec!["hyper".to_string()];
    assert!(check_transport_names(&names, false).is_ok());
}

#[tokio::test]
async fn mem_without_test_net_refused() {
    let err = bind_transports(&cfg_with(&["mem"], false), &secret())
        .await
        .err()
        .expect("expected an error");
    assert!(err.to_string().contains("for tests only"), "{err}");
}

#[tokio::test]
async fn unknown_name_refused() {
    let err = bind_transports(&cfg_with(&["iroh"], true), &secret())
        .await
        .err()
        .expect("expected an error");
    assert!(
        err.to_string().contains("is not built; available: hyper"),
        "{err}"
    );
}

#[tokio::test]
async fn empty_list_refused() {
    let err = bind_transports(&cfg_with(&[], true), &secret())
        .await
        .err()
        .expect("expected an error");
    assert!(
        err.to_string().contains("the transport list is empty"),
        "{err}"
    );
}

#[tokio::test]
async fn mem_with_test_net_binds() {
    let list = bind_transports(&cfg_with(&["mem"], true), &secret())
        .await
        .unwrap()
        .list;
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

/// The one-off DHT (redeeming an invite) never reads or writes a table file.
#[cfg(feature = "net-hyper")]
#[test]
fn ephemeral_dht_has_no_table_file() {
    assert!(
        ephemeral_opts().table_file.is_none(),
        "a one-off DHT must not touch the saved table"
    );
}

/// The config's `dht_table_path` reaches the transport: closing it leaves the table file behind.
#[cfg(feature = "net-hyper")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bind_with_passes_table_file_to_hyper() {
    // given: a config naming a table file, on a loopback DHT cluster
    let dir = scratch("bind-with");
    let file = dir.join("dht-nodes.txt");
    let cluster = crate::transport::net_hyper::test_cluster::cluster(3).await;
    let mut cfg = cfg_with(&["hyper"], true);
    cfg.dht_table_path = Some(file.clone());
    let opts = BindOpts {
        hyper: crate::transport::net_hyper::HyperOpts {
            bootstrap: Some(cluster.bootstrap()),
            ..Default::default()
        },
    };
    // when: the transports are bound and the hyper one is closed
    let bound = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        bind_with(&cfg, &secret(), &opts),
    )
    .await
    .expect("bind did not finish in time")
    .expect("bind_with");
    bound.hyper.expect("typed hyper handle").close().await;
    // then: the file exists and holds at least one node address
    let text = std::fs::read_to_string(&file).expect("the table file is written on close");
    let nodes = text
        .lines()
        .filter(|l| l.parse::<std::net::SocketAddrV4>().is_ok());
    assert!(
        nodes.count() >= 1,
        "the file must list at least one node: {text:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fresh temporary directory for one test.
#[cfg(feature = "net-hyper")]
fn scratch(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-start-transports-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}
