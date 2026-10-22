//! Tests of domain parsing and the node's list (`domains.rs`); the shared document and reconciliation are in `core/tests/domains_loopback.rs`.

use super::*;

#[test]
fn normalize_accepts_and_lowercases() {
    assert_eq!(normalize_domain("  Example.COM. ").unwrap(), "example.com");
    assert_eq!(normalize_domain("a-b.c1.org").unwrap(), "a-b.c1.org");
    assert_eq!(
        normalize_domain("xn--r8jz45g.xn--zckzah").unwrap(),
        "xn--r8jz45g.xn--zckzah"
    );
    let long_ok = format!("{}.com", "a".repeat(63));
    assert_eq!(normalize_domain(&long_ok).unwrap(), long_ok);
}

#[test]
fn normalize_rejects_bad() {
    let too_long = vec!["a".repeat(63); 4].join(".");
    let long_label = format!("{}.com", "a".repeat(64));
    for bad in [
        "",
        " . ",
        "com",
        "a..b",
        "-a.com",
        "a-.com",
        "a_b.com",
        "a b.com",
        "https://",
        "http://a b.com",
        "mailto:",
        too_long.as_str(),
        long_label.as_str(),
    ] {
        assert!(
            normalize_domain(bad).is_err(),
            "should be rejected: \"{bad}\""
        );
    }
}

#[test]
fn entry_key_has_terminator() {
    assert_eq!(entry_key("a.com"), b"list/a.com/".to_vec());
    assert!(!entry_key("a.com.org").starts_with(&entry_key("a.com")));
    assert!(!entry_key("a.com").starts_with(&entry_key("a.co")));
    assert_eq!(
        domain_from_key(&entry_key("a.com")),
        Some("a.com".to_string())
    );
}

#[test]
fn domain_from_key_rejects_foreign() {
    for key in [
        b"chk/a.com/".as_slice(),
        b"list/a.com".as_slice(),
        b"list/A.com/".as_slice(),
        b"list//".as_slice(),
        b"list/\xff.com/".as_slice(),
    ] {
        assert_eq!(domain_from_key(key), None, "key {key:?}");
    }
}

#[test]
fn console_parse() {
    assert_eq!(
        parse_domain_console_line("domains add Example.com").unwrap(),
        Some(DomainConsoleCommand::Add("Example.com".to_string()))
    );
    assert_eq!(
        parse_domain_console_line("  domains   remove a.com ").unwrap(),
        Some(DomainConsoleCommand::Remove("a.com".to_string()))
    );
    assert_eq!(
        parse_domain_console_line("domains list").unwrap(),
        Some(DomainConsoleCommand::List)
    );
    assert_eq!(parse_domain_console_line("   ").unwrap(), None);
    for bad in [
        "domains",
        "domains add",
        "domains add a.com b.com",
        "list",
        "add a.com",
    ] {
        assert!(
            parse_domain_console_line(bad).is_err(),
            "should be rejected: \"{bad}\""
        );
    }
}

#[test]
fn normalize_extracts_host_from_url() {
    for (input, want) in [
        ("https://www.Example.ORG/path?x=1#y", "www.example.org"),
        ("http://user:pw@site.net:8080/", "site.net"),
        ("example.org/login", "example.org"),
        ("site.net:443", "site.net"),
        (" example.com. ", "example.com"),
        ("https://bücher.example/", "xn--bcher-kva.example"),
        ("例え.テスト", "xn--r8jz45g.xn--zckzah"),
        ("1.2.3.4", "1.2.3.4"),
    ] {
        assert_eq!(normalize_domain(input).unwrap(), want, "input \"{input}\"");
    }
}

mod shared_replica {
    //! The shared list replica on the `mem` sync: the list is assembled from `open_replica` of both replicas.
    use super::*;
    use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
    use crate::transport::net::sync_backend::{SyncBackend, SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
    use crate::transport::net::{NodeSecret, ReplicaEntry};
    use bytes::Bytes;

    struct Rig {
        list: DomainList,
        secret: NodeSecret,
        shared: Arc<MemReplica>,
        _keep: (Arc<MemSyncNet>, Arc<MemSync>),
    }

    async fn rig() -> Rig {
        let net = MemSyncNet::new();
        let secret = NodeSecret::from_bytes(&[3; 32]);
        let ctx = SyncCtx {
            secret: secret.clone(),
            network_seed: [9; 32],
            store_dir: None,
        };
        let backend = net.backend(ctx);
        let shared = backend.open_replica(REPLICA_NET).await.unwrap();
        let own = backend.open_replica(REPLICA_DOMAINS).await.unwrap();
        let list = DomainList::open(shared, own).await.unwrap();
        Rig {
            list,
            secret,
            shared: backend.replica_mem(REPLICA_NET).unwrap(),
            _keep: (net, backend),
        }
    }

    #[tokio::test]
    async fn me_is_node_peer_id() {
        let r = rig().await;
        assert_eq!(r.list.me(), r.secret.peer_id());
    }

    #[tokio::test]
    async fn replica_roundtrip() {
        let r = rig().await;
        r.list
            .shared_replica()
            .set_own(b"k/1", Bytes::from_static(b"val"))
            .await
            .unwrap();
        let e = r
            .list
            .shared_replica()
            .get(r.list.me(), b"k/1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e.value, Bytes::from_static(b"val"));
    }

    #[tokio::test]
    async fn replica_sees_raw_writes() {
        let r = rig().await;
        r.shared.put_raw(ReplicaEntry {
            author: r.list.me(),
            key: b"k/2".to_vec(),
            value: Bytes::from_static(b"old"),
            ts_micros: 1_700_000_000_000_000,
        });
        let e = r
            .list
            .shared_replica()
            .get(r.list.me(), b"k/2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e.value, Bytes::from_static(b"old"));
    }
}
