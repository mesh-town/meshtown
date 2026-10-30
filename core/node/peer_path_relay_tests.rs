//! The `[relay]` line for a new connection (`node/peer_path.rs`): only relayed paths produce one.

use super::relay_connect_line;
use crate::transport::net::contract::pid;
use crate::transport::net::{PathKind, RelayRef};

#[test]
fn relayed_by_member_names_the_relay_member() {
    // given: a path through a relay run by a member
    let path = PathKind::Relayed(RelayRef::Peer(pid(2)));
    // when
    let line = relay_connect_line("alice", &path, |_| "bob".to_string());
    // then
    assert_eq!(line.as_deref(), Some("connected to alice via relay of bob"));
}

#[test]
fn relayed_by_url_names_the_url() {
    // given
    let path = PathKind::Relayed(RelayRef::Url("https://relay.example".into()));
    // when
    let line = relay_connect_line("alice", &path, |_| unreachable!());
    // then
    assert_eq!(
        line.as_deref(),
        Some("connected to alice via relay of https://relay.example")
    );
}

#[test]
fn relayed_by_unnamed_relay_says_relay_server() {
    // given: a path through a relay nobody knows the owner of (the receiving side)
    let path = PathKind::Relayed(RelayRef::Unnamed);
    // when
    let line = relay_connect_line("alice", &path, |_| unreachable!());
    // then
    let line = line.expect("a relayed path has a line");
    assert!(
        line.starts_with("connected to alice via ") && line.ends_with("a relay server"),
        "line is {line:?}"
    );
}

#[test]
fn direct_and_unknown_paths_are_silent() {
    for path in [PathKind::Direct, PathKind::Unknown] {
        assert_eq!(relay_connect_line("alice", &path, |_| String::new()), None);
    }
}
