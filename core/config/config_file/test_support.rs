//! Shared helpers for the `config_file/*` tests: a temporary directory, sample settings and secret,
//! reading the resulting file. Visibility `pub(super)` — as before within one file, now
//! within all of `config_file/`, no wider.

use crate::transport::net::{NodeSecret, PeerId};
use std::path::{Path, PathBuf};

use super::settings::Settings;
use super::{LoadedConfigFile, ParsedConfigFile, SecretRead};
use super::{NetSettings, ProcessSettings};
use crate::ports_sharing::types::Proto;

/// A temporary test directory; removed with `remove_dir_all` at the end of the test.
pub(super) fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "mesh-cfg-{name}-{}-{nanos}-{seq}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("couldn't create the test's temporary directory");
    dir
}

pub(super) fn zero_secret_b64() -> String {
    data_encoding::BASE64.encode(&[0u8; 32])
}

pub(super) fn test_secret() -> NodeSecret {
    NodeSecret::from_bytes(&[7u8; 32])
}

/// Returns settings where no field has its default value.
pub(super) fn sample_settings() -> Settings {
    Settings {
        process: ProcessSettings {
            node_memory_warn_mib: 300,
            node_memory_exit_mib: 700,
            socks_enabled: false,
            socks_port: 1090,
            socks_access: super::ListenAccess::Localnet,
            socks_ifaces: super::IfaceFilter::Only(vec!["br-lan".to_string()]),
            proxy_net: super::NetSlotId::parse("ab2c3d").expect("valid id"),
            web_ifaces: super::IfaceFilter::Any,
            web_enabled: false,
            web_port: 8080,
            web_access: super::ListenAccess::Localnet,
            log_ring_lines: 500,
            autostart_enabled: false,
            full_tunnel_enabled: false,
        },
        net: NetSettings {
            node_name: Some("ginger-pool".to_string()),
            network_key: Some("鍵🔑--test-key-not-real".to_string()),
            exit_enabled: false,
            checks_enabled: false,
            check_interval_sec: 900,
            doh_server: "9.9.9.9:443".parse().expect("address"),
            provider_dns: Some("8.8.8.8:53".parse().expect("address")),
            geo_markers: vec![],
            probe_interval_sec: 120,
            probe_full_cycle: false,
            route_exits: vec![NodeSecret::from_bytes(&[9u8; 32]).peer_id()],
            ports_published: vec![crate::ports_sharing::types::Published {
                proto: Proto::Tcp,
                port: 8080,
                name: "網".to_string(),
                audience: crate::ports_sharing::types::Audience::All,
            }],
            ports_opened: vec![],
            net_choice: crate::node::net_names::default_choice(),
        },
    }
}

fn peer_of(b: u8) -> PeerId {
    NodeSecret::from_bytes(&[b; 32]).peer_id()
}

fn changed_published() -> Vec<crate::ports_sharing::types::Published> {
    let peer = peer_of;
    vec![
        crate::ports_sharing::types::Published {
            proto: Proto::Tcp,
            port: 8080,
            name: "網".to_string(),
            audience: crate::ports_sharing::types::Audience::All,
        },
        crate::ports_sharing::types::Published {
            proto: Proto::Tcp,
            port: 9090,
            name: "二番".to_string(),
            audience: crate::ports_sharing::types::Audience::Nodes(vec![peer(11)]),
        },
    ]
}

/// Returns settings where EVERY field differs from `Settings::default()`, except `net.transports`
/// (only `hyper` is built, and it is also the default). The literal is complete, without `..`:
/// a new field in the `Settings` struct breaks compilation until it is added here.
pub(crate) fn all_changed() -> Settings {
    let peer = peer_of;
    Settings {
        process: ProcessSettings {
            node_memory_warn_mib: 500,
            node_memory_exit_mib: 800,
            socks_enabled: false,
            socks_port: 1090,
            socks_access: super::ListenAccess::Localnet,
            socks_ifaces: super::IfaceFilter::Only(vec!["br-lan".to_string()]),
            proxy_net: super::NetSlotId::parse("ab2c3d").expect("valid id"),
            web_ifaces: super::IfaceFilter::Any,
            web_enabled: false,
            web_port: 8080,
            web_access: super::ListenAccess::Localhost,
            log_ring_lines: 500,
            autostart_enabled: true,
            full_tunnel_enabled: true,
        },
        net: NetSettings {
            node_name: Some("台所 \"1\"".to_string()),
            network_key: Some("鍵🔑--test-key-not-real".to_string()),
            exit_enabled: false,
            checks_enabled: false,
            check_interval_sec: 600,
            doh_server: "9.9.9.9:443".parse().expect("address"),
            provider_dns: Some("[::1]:53".parse().expect("address")),
            geo_markers: vec!["ünavailable hère".to_string()],
            probe_interval_sec: 120,
            probe_full_cycle: true,
            route_exits: vec![peer(9), peer(10)],
            ports_published: changed_published(),
            ports_opened: vec![crate::ports_sharing::types::Opened {
                proto: Proto::Tcp,
                node: peer(12),
                port: 80,
                local: 8081,
            }],
            net_choice: crate::transport::net::NetChoice {
                transports: vec!["hyper".to_string()],
                discovery: vec!["cache".to_string(), "invite".to_string()],
                sync: "mem".to_string(),
            },
        },
    }
}

pub(super) fn parsed(path: &Path) -> ParsedConfigFile {
    match super::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p,
        other => panic!("expected Parsed, got {other:?}"),
    }
}

pub(super) fn secret_of(p: &ParsedConfigFile) -> [u8; 32] {
    match &p.secret {
        SecretRead::Valid(k) => k.to_bytes(),
        other => panic!("expected Valid, got {other:?}"),
    }
}

pub(super) fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Clears the "read-only" bit set by a test so that `remove_dir_all` at the end
/// of the test does not trip over it. A test throwaway, so widening permissions is not a problem.
#[allow(clippy::permissions_set_readonly_false)]
pub(super) fn clear_readonly(file: &Path) {
    if let Ok(md) = std::fs::metadata(file) {
        let mut perms = md.permissions();
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(file, perms);
    }
}
