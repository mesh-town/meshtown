//! `Held`: a slot in the dial queue is taken until the first completed read and no longer.

use super::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::Semaphore;
use tokio::time::timeout;

/// A one-slot semaphore, already taken by a `Held` stream over one end of a `duplex`; the other end is handed out.
async fn held_pair() -> (Arc<Semaphore>, Held<DuplexStream>, DuplexStream) {
    let slots = Arc::new(Semaphore::new(1));
    let (near, far) = duplex(64);
    let permit = slots.clone().acquire_owned().await.unwrap();
    (slots, Held::new(near, permit), far)
}

#[tokio::test]
async fn slot_is_taken_while_held_lives_and_nothing_was_read() {
    // given: Held is created; when: there were no reads; then: no free slots
    let (slots, held, _far) = held_pair().await;
    assert_eq!(slots.available_permits(), 0);
    drop(held);
}

#[tokio::test]
async fn first_read_with_data_releases_the_slot() {
    // given: the other side sent a byte; when: Held reads; then: the slot is free, and Held is alive
    let (slots, mut held, mut far) = held_pair().await;
    far.write_all(b"x").await.unwrap();
    let mut got = [0u8; 1];
    held.read_exact(&mut got).await.unwrap();
    assert_eq!(slots.available_permits(), 1);
    drop(held);
}

#[tokio::test]
async fn first_read_hitting_eof_releases_the_slot() {
    // given: the other side closed the stream; when: Held reads the end; then: the slot is free
    let (slots, mut held, far) = held_pair().await;
    drop(far);
    let mut got = [0u8; 1];
    assert_eq!(held.read(&mut got).await.unwrap(), 0);
    assert_eq!(slots.available_permits(), 1);
    drop(held);
}

#[tokio::test]
async fn dropping_without_any_read_releases_the_slot() {
    // given: Held without reads; when: it is dropped; then: the slot is free
    let (slots, held, _far) = held_pair().await;
    drop(held);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn writing_does_not_release_the_slot() {
    // given: Held; when: written to and flushed; then: the slot is still taken
    let (slots, mut held, _far) = held_pair().await;
    held.write_all(b"hello").await.unwrap();
    held.flush().await.unwrap();
    assert_eq!(slots.available_permits(), 0);
    drop(held);
}

#[tokio::test]
async fn pending_read_does_not_release_the_slot() {
    // given: no data; when: a read waits 50 ms; then: the slot is taken
    let (slots, mut held, _far) = held_pair().await;
    let mut got = [0u8; 1];
    let waited = timeout(Duration::from_millis(50), held.read(&mut got)).await;
    assert!(waited.is_err());
    assert_eq!(slots.available_permits(), 0);
    drop(held);
}
