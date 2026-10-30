//! `join` with a network key, against a running node and without one.
//!
//! What it does: runs the real binary through `CliNode` (own settings file, own page port) and checks the exit
//! code, the label on stdout and what ends up in the settings file.
//! What it doesn't do: the exchange of an invite with network members (covered by the invite tests).

#[path = "common/cli_node.rs"]
mod cli_node;

use cli_node::{cli_in, free_port, scratch_dir, stderr, stdout, CliNode};

use meshtown_core::members::network_key::managed_key_with_label;
use meshtown_core::transport::net::NodeSecret;

fn key_with_label(label: &str) -> String {
    let creator = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    managed_key_with_label(Some(label), &[9u8; 32], &creator)
}

#[test]
fn join_key_on_live_node_prints_label() {
    // given: a running node without a network
    let node = CliNode::start(&[]);
    let key = key_with_label("our-room");
    // when
    let out = node.cli(&["join", &key]);
    // then: exit 0, the label alone on stdout
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "our-room");
    assert!(stderr(&out).contains("Joined"), "{}", stderr(&out));
}

#[test]
fn join_key_without_node_writes_file() {
    // given: no node on the port, no settings file
    let dir = scratch_dir("join-file");
    let key = key_with_label("our-room");
    // when
    let out = cli_in(&["join", &key], &dir, free_port());
    // then: exit 0, the label on stdout, the key in the file
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "our-room");
    let file = std::fs::read_to_string(dir.join("config.json")).expect("settings file");
    assert!(file.contains(&key), "{file}");
    assert!(stderr(&out).contains("Saved to"), "{}", stderr(&out));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn join_same_key_again_says_already() {
    // given: a running node that has joined the network of the key
    let node = CliNode::start(&[]);
    let key = key_with_label("our-room");
    let first = node.cli(&["join", &key]);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    // when: the same key is joined again
    let out = node.cli(&["join", &key]);
    // then: exit 0, "already", nothing reconnects
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("already in this network"),
        "{}",
        stderr(&out)
    );
    assert!(!stderr(&out).contains("Joined"), "{}", stderr(&out));
}

#[test]
fn join_garbage_is_a_usage_error() {
    let node = CliNode::start(&[]);
    let out = node.cli(&["join", "garbage string"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("invite"), "{}", stderr(&out));
    assert!(!stderr(&out).contains("--key-only"), "{}", stderr(&out));
}

#[test]
fn join_help_names_the_default_once() {
    let node = CliNode::start(&[]);
    let out = node.cli(&["join", "--help"]);
    let text = stdout(&out);
    assert_eq!(text.matches("120").count(), 1, "{text}");
}
