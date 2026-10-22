//! Session table: routing an incoming stream to its session.

use super::*;
use crate::net::id::NodeSecret;
use crate::net::stream::StreamPair;
use crate::net_hyper::wire::{REFUSE_SESSION_CLOSED, REFUSE_NO_SESSION};
use tokio::io::duplex;
use tokio::sync::mpsc;

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn key(peer: PeerId, session: u64, numbered_by_us: bool) -> SessionKey {
    SessionKey { peer, session, numbered_by_us }
}

fn pair() -> StreamPair {
    let (a, _b) = duplex(64);
    let (r, w) = tokio::io::split(a);
    StreamPair { send: Box::new(w), recv: Box::new(r) }
}

/// A helper in place of the removed `Sessions::route`: a queue slot, then sending the stream.
fn route(table: &Sessions, key: SessionKey, pair: StreamPair) -> Result<(), u32> {
    let permit = table.reserve(key)?;
    permit.send(pair);
    Ok(())
}

#[tokio::test]
async fn route_reaches_the_inserted_session() {
    let table = Sessions::new();
    let (tx, mut rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Ok(()));
    assert!(rx.recv().await.is_some());
}

#[test]
fn route_to_unknown_session_gives_no_session() {
    let table = Sessions::new();
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Err(REFUSE_NO_SESSION));
}

#[test]
fn same_number_from_another_peer_does_not_match() {
    let table = Sessions::new();
    let (tx, _rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    assert_eq!(route(&table, key(peer(2), 7, true), pair()), Err(REFUSE_NO_SESSION));
    assert_eq!(route(&table, key(peer(1), 7, false), pair()), Err(REFUSE_NO_SESSION));
}

#[test]
fn route_after_receiver_dropped_gives_closed_and_forgets() {
    let table = Sessions::new();
    let (tx, rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    drop(rx);
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Err(REFUSE_SESSION_CLOSED));
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Err(REFUSE_NO_SESSION));
}

#[test]
fn remove_forgets_the_session() {
    let table = Sessions::new();
    let (tx, _rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    table.remove(key(peer(1), 7, true));
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Err(REFUSE_NO_SESSION));
}

#[tokio::test]
async fn close_all_empties_the_table() {
    let table = Sessions::new();
    let (tx, mut rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    table.close_all();
    assert_eq!(route(&table, key(peer(1), 7, true), pair()), Err(REFUSE_NO_SESSION));
    assert!(rx.recv().await.is_none(), "the sender must be dropped");
}

#[tokio::test]
async fn reserved_place_delivers_the_stream_later() {
    let table = Sessions::new();
    let (tx, mut rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    let permit = table.reserve(key(peer(1), 7, true)).expect("slot is taken");
    permit.send(pair());
    assert!(rx.recv().await.is_some());
}

#[test]
fn reserve_for_unknown_or_dropped_session_gives_a_code() {
    let table = Sessions::new();
    assert_eq!(table.reserve(key(peer(1), 7, true)).err(), Some(REFUSE_NO_SESSION));
    let (tx, rx) = mpsc::channel(4);
    table.insert(key(peer(1), 7, true), tx);
    drop(rx);
    assert_eq!(table.reserve(key(peer(1), 7, true)).err(), Some(REFUSE_SESSION_CLOSED));
    assert_eq!(table.reserve(key(peer(1), 7, true)).err(), Some(REFUSE_NO_SESSION));
}
