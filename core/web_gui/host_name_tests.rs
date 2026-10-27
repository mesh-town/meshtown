//! Tests for `host_name.rs`.

use super::*;

#[test]
fn accepts_label_with_and_without_port() {
    for ok in [
        "node-1.meshtown",
        "node-1.meshtown:7777",
        "a.meshtown:1",
        "A9.MeshTown:65535",
        "-.meshtown",
    ] {
        assert!(is_mesh_host_name(ok), "{ok}");
    }
}

#[test]
fn accepts_longest_label_only() {
    assert!(is_mesh_host_name(&format!("{}.meshtown", "a".repeat(63))));
    assert!(!is_mesh_host_name(&format!("{}.meshtown", "a".repeat(64))));
}

#[test]
fn rejects_other_domains_and_shapes() {
    let bad = [
        "localhost",
        "localhost:7777",
        "x.meshtown.evil",
        "a.b.meshtown",
        ".meshtown",
        "meshtown",
        "a.meshtown.",
        "a_b.meshtown",
        "a b.meshtown",
        "127.0.0.1:7777",
        "a.meshtown:",
        "a.meshtown:80:80",
        "a.meshtown/x",
        "a@b.meshtown",
        "",
    ];
    for host in bad {
        assert!(!is_mesh_host_name(host), "{host}");
    }
}

#[test]
fn rejects_bad_ports() {
    for port in ["0", "65536", "99999", "123456", "-1", "+80", "8o", " 80"] {
        assert!(!is_mesh_host_name(&format!("a.meshtown:{port}")), "{port}");
    }
}

#[test]
fn loopback_clients_only() {
    for ok in ["127.0.0.1", "127.9.9.9", "::1", "::ffff:127.0.0.1"] {
        assert!(is_loopback_client(ok.parse().unwrap()), "{ok}");
    }
    for no in [
        "192.168.1.5",
        "10.0.0.1",
        "100.64.0.1",
        "8.8.8.8",
        "fe80::1",
    ] {
        assert!(!is_loopback_client(no.parse().unwrap()), "{no}");
    }
}
