//! `join` with no node running: the key goes into the settings file and a node started on that file is in the
//! network; while the command waits for an invite it holds the page port against a node that starts meanwhile.

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/join_fixtures.rs"]
mod fixtures;

use std::io::Read;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use cli_node::{cli_in, free_port, scratch_dir, spawn_cli, stderr, stdout, CliNode};
use fixtures::{key_with_label, unanswered_invite};
use serde_json::Value;

#[test]
fn key_saved_without_a_node_is_used_by_the_next_start() {
    // given: no node and no settings file; the join writes the key
    let dir = scratch_dir("join-then-run");
    let key = key_with_label("our-room", 9);
    let out = cli_in(&["join", &key], &dir, free_port());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "our-room");
    // when: a node starts on that settings file
    let node = CliNode::start_in(dir, &[]);
    // then: it is in the network without being asked to join
    let status = node.cli(&["status", "--json"]);
    let reply: Value = serde_json::from_str(stdout(&status).trim()).expect("one JSON line");
    let first = &reply["nets"][0];
    assert!(
        first["id_hex"].as_str().is_some_and(|h| !h.is_empty()),
        "{reply}"
    );
    assert_eq!(first["label"], "our-room", "{reply}");
}

#[test]
fn waiting_join_holds_the_page_port() {
    // given: `join` of an invite nobody answers, running without a node on a port of its own
    let (dir, other) = (scratch_dir("join-hold"), scratch_dir("join-hold-2"));
    let port = free_port();
    let invite = unanswered_invite(&key_with_label("our-room", 9));
    let mut join = spawn_cli(&["join", &invite, "--timeout", "60"], &dir, port);
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "the join did not take the port");
        std::thread::sleep(Duration::from_millis(100));
    }
    // when: a node is started on the same port
    let mut node = spawn_cli(&["run", "--no-checks", "--no-socks"], &other, port);
    let started = Instant::now();
    let exit = loop {
        if let Some(status) = node.try_wait().expect("wait") {
            break Some(status);
        }
        if started.elapsed() > Duration::from_secs(20) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // then: it did not start
    let mut said = String::new();
    if exit.is_some() {
        let _ = node.stderr.take().unwrap().read_to_string(&mut said);
    }
    let _ = (join.kill(), node.kill());
    let _ = (join.wait(), node.wait());
    let _ = (
        std::fs::remove_dir_all(&dir),
        std::fs::remove_dir_all(&other),
    );
    assert!(
        exit.is_some_and(|s| !s.success()),
        "the second node started on a held port"
    );
    assert!(said.contains("in use"), "{said}");
}
