//! The client commands of the real binary (`status`, `stop`, `leave`, the global options) against a node started
//! by `CliNode` on its own settings file and a page port chosen by the system.

#[path = "common/cli_node.rs"]
mod cli_node;

use std::time::Duration;

use cli_node::{free_port, scratch_dir, stderr as err, stdout as out, CliNode};

#[cfg(not(feature = "gui"))]
#[test]
fn no_args_prints_help_exit_0() {
    let dir = scratch_dir("no-args");
    let o = cli_node::cli_no_args(&dir);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(out(&o).contains("Global options:"), "stdout: {}", out(&o));
    assert!(err(&o).is_empty(), "stderr: {}", err(&o));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn status_without_node_exits_3() {
    let dir = scratch_dir("status-off");
    let o = cli_node::cli_in(&["status"], &dir, free_port());
    assert_eq!(o.status.code(), Some(3), "stdout: {}", out(&o));
    assert!(err(&o).contains("isn't running"), "stderr: {}", err(&o));
    assert!(out(&o).is_empty(), "stdout: {}", out(&o));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn status_json_on_keyless_node_has_nets() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["status", "--json"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    let reply: serde_json::Value = serde_json::from_str(out(&o).trim()).expect("one JSON line");
    assert!(reply["nets"].is_array(), "{reply}");
    let plain = node.cli(&["status"]);
    assert_eq!(plain.status.code(), Some(0), "stderr: {}", err(&plain));
    assert!(
        out(&plain).starts_with("(no network)\t"),
        "stdout: {}",
        out(&plain)
    );
    assert!(err(&plain).contains("join"), "stderr: {}", err(&plain));
}

#[test]
fn stop_ends_the_node_then_status_is_3() {
    let mut node = CliNode::start(&[]);
    let o = node.cli(&["stop"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("Mesh Town stopped."),
        "stderr: {}",
        err(&o)
    );
    assert!(
        node.wait_exit(Duration::from_secs(15)).is_some(),
        "the node is still running"
    );
    assert_eq!(node.cli(&["status"]).status.code(), Some(3));
}

#[test]
fn unused_global_is_usage_error() {
    let dir = scratch_dir("unused");
    let o = cli_node::cli_in(&["stop", "--json"], &dir, free_port());
    assert_eq!(o.status.code(), Some(2), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("--json isn't used by"),
        "stderr: {}",
        err(&o)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn leave_without_yes_is_usage_error() {
    let dir = scratch_dir("leave-no-yes");
    let o = cli_node::cli_in(&["leave"], &dir, free_port());
    assert_eq!(o.status.code(), Some(2), "stderr: {}", err(&o));
    assert!(err(&o).contains("--yes"), "stderr: {}", err(&o));
    let _ = std::fs::remove_dir_all(dir);
}
