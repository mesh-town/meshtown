//! The `access` group of the real binary: `access`, `access page`, `access proxy` and `access ifaces`, against a
//! running node (a real `run` process) and against a bare settings file.

#[path = "common/cli_node.rs"]
mod cli_node;

use std::path::{Path, PathBuf};

use meshtown_core::config::config_file::{new_secret, save_config_file, Settings};
use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
use meshtown_core::transport::net::NodeSecret;
use serde_json::Value;

use cli_node::{cli_in as mesh, free_port, scratch_dir, stderr as err, stdout as out, CliNode};

/// A fresh directory only its owner can enter, so the commands have nothing to warn about.
fn private_dir(tag: &str) -> PathBuf {
    let dir = scratch_dir(tag);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    dir
}

/// A valid managed network key of a fresh base.
fn good_key() -> String {
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    managed_key_with_label(None, &base, &root)
}

/// A directory with a settings file at revision 1 and no node behind it.
fn seeded(tag: &str) -> PathBuf {
    let dir = private_dir(tag);
    let secret = new_secret().expect("secret");
    save_config_file(
        &dir.join("config.json"),
        None,
        1,
        &secret,
        &Settings::default(),
    )
    .expect("settings file");
    dir
}

fn config_json(dir: &Path) -> Value {
    let text = std::fs::read_to_string(dir.join("config.json")).expect("config.json");
    serde_json::from_str(&text).expect("JSON")
}

const PAGE_LOCALHOST: &str = r#"{"config":{"version":1,"web":{"access":"localhost"}}}"#;

fn lines(o: &std::process::Output) -> Vec<String> {
    out(o).lines().map(str::to_string).collect()
}

#[test]
fn page_live_localhost_is_immediate() {
    let node = CliNode::start_with_config(PAGE_LOCALHOST, &[]);
    let o = node.cli(&["access", "page", "localhost"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("takes effect immediately"),
        "stderr: {}",
        err(&o)
    );
    assert_eq!(out(&o), "", "a write prints nothing on stdout");
}

#[test]
fn page_live_localnet_needs_restart() {
    let node = CliNode::start_with_config(PAGE_LOCALHOST, &[]);
    let o = node.cli(&["access", "page", "localnet"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("after Mesh Town restarts"),
        "stderr: {}",
        err(&o)
    );
    assert_eq!(out(&o), "");
    let shown = node.cli(&["access"]);
    assert_eq!(lines(&shown)[0], "page localnet", "stdout: {}", out(&shown));
}

#[test]
fn page_offline_writes_field_and_bumps_rev() {
    let dir = seeded("access-off");
    let before = config_json(&dir);
    let o = mesh(&["access", "page", "localhost"], &dir, free_port());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("applies at startup"),
        "stderr: {}",
        err(&o)
    );
    assert_eq!(out(&o), "");
    let after = config_json(&dir);
    assert_eq!(after["config"]["web"]["access"], "localhost", "{after}");
    assert!(
        after["rev"].as_u64() > before["rev"].as_u64(),
        "{before} -> {after}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn offline_without_file_is_error_code_1() {
    let dir = scratch_dir("access-nofile");
    let o = mesh(&["access", "page", "localhost"], &dir, free_port());
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    assert!(err(&o).contains("no settings file"), "stderr: {}", err(&o));
    assert!(!dir.join("config.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bogus_mode_is_code_2() {
    let dir = scratch_dir("access-bogus");
    let o = mesh(&["access", "page", "everyone"], &dir, free_port());
    assert_eq!(o.status.code(), Some(2), "stdout: {}", out(&o));
    let text = err(&o);
    assert!(
        text.contains("localhost") && text.contains("localnet"),
        "stderr: {text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The page answers before the network is up; the restart is refused with "is starting" until then.
fn restart_when_ready(node: &CliNode) -> std::process::Output {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let o = node.cli(&["access", "proxy", "localnet"]);
        if !err(&o).contains("is starting") || std::time::Instant::now() > deadline {
            return o;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

#[test]
fn proxy_live_restarts_real_node() {
    let port = free_port().to_string();
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{}"}}}}}}"#,
        good_key()
    );
    let node = CliNode::start_with_config(&config, &["--socks", "--socks-port", &port]);
    let same = node.cli(&["access", "proxy", "localhost"]);
    assert_eq!(same.status.code(), Some(0), "stderr: {}", err(&same));
    assert!(err(&same).contains("unchanged"), "stderr: {}", err(&same));
    let o = restart_when_ready(&node);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(err(&o).contains("proxy restarted"), "stderr: {}", err(&o));
    let shown = node.cli(&["access"]);
    assert_eq!(shown.status.code(), Some(0), "stderr: {}", err(&shown));
    assert!(
        lines(&shown).contains(&"proxy localnet".to_string()),
        "{}",
        out(&shown)
    );
}

#[test]
fn proxy_offline_writes_socks_access() {
    let dir = seeded("access-proxy-off");
    let before = config_json(&dir);
    let o = mesh(&["access", "proxy", "localnet"], &dir, free_port());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("proxy access: localnet"),
        "stderr: {}",
        err(&o)
    );
    let after = config_json(&dir);
    assert_eq!(after["config"]["socks"]["access"], "localnet", "{after}");
    assert_eq!(after["config"]["web"], before["config"]["web"], "{after}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ifaces_live_writes_both_fields() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["access", "ifaces", "br-lan", "lo"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("Interface list saved"),
        "stderr: {}",
        err(&o)
    );
    let doc = config_json(&node.dir);
    for section in ["socks", "web"] {
        assert_eq!(
            doc["config"][section]["ifaces"],
            serde_json::json!(["br-lan", "lo"]),
            "{doc}"
        );
    }
}

#[test]
fn ifaces_if_unset_creates_file_when_absent() {
    let dir = private_dir("access-seed");
    let o = mesh(
        &["access", "ifaces", "br0", "--if-unset"],
        &dir,
        free_port(),
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert_eq!(out(&o), "");
    let doc = config_json(&dir);
    assert_eq!(
        doc["config"]["web"]["ifaces"],
        serde_json::json!(["br0"]),
        "{doc}"
    );
    assert_eq!(
        doc["config"]["socks"]["ifaces"],
        serde_json::json!(["br0"]),
        "{doc}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ifaces_if_unset_skips_when_key_present() {
    let dir = seeded("access-skip");
    let before = std::fs::read(dir.join("config.json")).unwrap();
    let o = mesh(
        &["access", "ifaces", "br0", "--if-unset"],
        &dir,
        free_port(),
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("Interface list unchanged"),
        "stderr: {}",
        err(&o)
    );
    assert_eq!(out(&o), "");
    assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ifaces_if_unset_runs_without_node_on_busy_port() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    let port = busy.local_addr().unwrap().port();
    let dir = private_dir("access-busy");
    let started = std::time::Instant::now();
    let o = mesh(&["access", "ifaces", "br0", "--if-unset"], &dir, port);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "waited on the port"
    );
    assert!(dir.join("config.json").exists());
    drop(busy);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wrong_token_never_falls_back_to_file() {
    let node = CliNode::start(&[]);
    let other = scratch_dir("access-token");
    std::fs::copy(node.dir.join("config.json"), other.join("config.json")).expect("copy");
    let before = std::fs::read(other.join("config.json")).unwrap();
    let o = mesh(&["access", "page", "localhost"], &other, node.port);
    assert_eq!(
        o.status.code(),
        Some(1),
        "stdout: {} stderr: {}",
        out(&o),
        err(&o)
    );
    assert_eq!(std::fs::read(other.join("config.json")).unwrap(), before);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn show_offline_three_lines() {
    let dir = seeded("access-show");
    let o = mesh(&["access"], &dir, free_port());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert_eq!(
        lines(&o),
        ["page localnet", "proxy localhost", "ifaces auto"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn show_live_json_shape() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["access", "--json"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert_eq!(lines(&o).len(), 1, "stdout: {}", out(&o));
    let doc: Value = serde_json::from_str(&out(&o)).expect("one JSON line");
    for section in ["web", "socks"] {
        assert!(doc[section]["access"].is_string(), "{doc}");
        assert!(doc[section]["ifaces"].is_array(), "{doc}");
    }
    assert_eq!(doc.as_object().unwrap().len(), 2, "{doc}");
}

#[test]
fn write_commands_refuse_yes_json_net() {
    let dir = seeded("access-refuse");
    for flags in [&["--yes"][..], &["--json"], &["--net", "x"]] {
        let mut args = vec!["access", "page", "localhost"];
        args.extend_from_slice(flags);
        let o = mesh(&args, &dir, free_port());
        assert_eq!(o.status.code(), Some(2), "{flags:?}: {}", err(&o));
        assert!(err(&o).contains("isn't used by"), "{flags:?}: {}", err(&o));
    }
    assert_eq!(config_json(&dir)["rev"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_names_answer_code_2_with_replacement() {
    let dir = seeded("access-old");
    let cases = [
        (&["ifaces", "auto"][..], "access ifaces"),
        (&["access", "localhost"], "access page localhost"),
    ];
    for (args, replacement) in cases {
        let o = mesh(args, &dir, free_port());
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", err(&o));
        assert!(err(&o).contains(replacement), "{args:?}: {}", err(&o));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
