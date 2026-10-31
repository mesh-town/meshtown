//! The `config` group of the real binary: `config path`, `config get` and `config set`, against a bare settings
//! file and against a running node (a real `run` process on a free page port).

#[path = "common/cli_node.rs"]
mod cli_node;

use std::process::Output;

use serde_json::Value;

use cli_node::{cli_in as mesh, free_port, scratch_dir, stderr as err, stdout as out, CliNode};

/// A fresh directory only its owner can enter, so the commands have nothing to warn about.
fn private_dir(tag: &str) -> std::path::PathBuf {
    let dir = scratch_dir(tag);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    dir
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

#[test]
fn bare_file_path_set_and_get() {
    let dir = private_dir("config-bare");
    let port = free_port();
    let path = mesh(&["config", "path"], &dir, port);
    assert_eq!(code(&path), 0, "{}", err(&path));
    assert_eq!(
        out(&path).trim(),
        dir.join("config.json").display().to_string()
    );

    let set = mesh(&["config", "set", "probe.interval_sec", "20"], &dir, port);
    assert_eq!(code(&set), 0, "{}", err(&set));
    assert!(
        err(&set).contains("saved probe.interval_sec"),
        "{}",
        err(&set)
    );
    assert_eq!(out(&set), "");
    let get = mesh(&["config", "get", "probe.interval_sec"], &dir, port);
    assert_eq!(code(&get), 0, "{}", err(&get));
    assert_eq!(out(&get).trim(), "20");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_file_refusals_have_their_codes() {
    let dir = scratch_dir("config-refuse");
    let port = free_port();
    let unknown = mesh(&["config", "set", "nope.x", "1"], &dir, port);
    assert_eq!(code(&unknown), 2, "{}", err(&unknown));
    let read_only = mesh(&["config", "set", "web.port", "8000"], &dir, port);
    assert_eq!(code(&read_only), 1, "{}", err(&read_only));
    let key = mesh(&["config", "set", "network.key", "x"], &dir, port);
    assert_eq!(code(&key), 1, "{}", err(&key));
    assert!(err(&key).contains("join"), "{}", err(&key));
    assert!(!dir.join("config.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_file_with_net_is_not_running() {
    let dir = scratch_dir("config-net");
    let o = mesh(&["config", "get", "--net", "main"], &dir, free_port());
    assert_eq!(code(&o), 3, "{}", err(&o));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn live_node_set_get_and_refusals() {
    let node = CliNode::start(&[]);
    let set = node.cli(&["config", "set", "probe.interval_sec", "30"]);
    assert_eq!(code(&set), 0, "{}", err(&set));
    let get = node.cli(&["config", "get", "probe.interval_sec"]);
    assert_eq!(out(&get).trim(), "30", "{}", err(&get));

    let file_only = node.cli(&["config", "set", "probe.full_cycle", "true"]);
    assert_eq!(code(&file_only), 1);
    assert!(
        err(&file_only).contains("stop Mesh Town"),
        "{}",
        err(&file_only)
    );
}

#[test]
fn live_node_get_json_is_one_line_with_overrides() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["config", "get", "--json"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o).trim().lines().count(), 1);
    let body: Value = serde_json::from_str(out(&o).trim()).expect("JSON");
    let overrides = body["cli_overrides"].as_array().expect("cli_overrides");
    assert!(overrides.iter().any(|p| p == "checks.enabled"), "{body}");
}

#[test]
fn foreign_token_does_not_write_the_file() {
    let node = CliNode::start(&[]);
    let other = scratch_dir("config-foreign");
    let copy = other.join("config.json");
    std::fs::copy(node.dir.join("config.json"), &copy).expect("copy");
    let before = std::fs::read(&copy).expect("copy bytes");
    let o = mesh(
        &["config", "set", "probe.interval_sec", "40"],
        &other,
        node.port,
    );
    assert_eq!(code(&o), 1, "{}", err(&o));
    assert_eq!(std::fs::read(&copy).expect("copy bytes"), before);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn removed_run_flag_names_the_new_form() {
    let dir = scratch_dir("config-removed");
    let o = mesh(&["run", "--probe-interval", "20"], &dir, free_port());
    assert_eq!(code(&o), 2, "{}", err(&o));
    assert!(
        err(&o).contains("--set probe.interval_sec=<SECONDS>"),
        "{}",
        err(&o)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
