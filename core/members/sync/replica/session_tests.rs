//! Tests for the reconciliation session: two `State`s of one replica on `tokio::io::duplex`, the handshake skipped.

use super::*;
use crate::members::sync::replica::entry::seal;
use crate::transport::net::id::NodeSecret;
use crate::transport::net::replica::ReplicaId;
use tokio::io::{duplex, split};

fn rid() -> ReplicaId {
    ReplicaId([1; 32])
}

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn put(n: u8, key: &[u8], value: &[u8], ts: u64) -> WireEntry {
    seal(&secret(n), &rid(), key, value, ts)
}

fn state_with(me: u8, items: Vec<WireEntry>) -> Mutex<State> {
    let mut s = State::new(rid(), secret(me).peer_id());
    for w in items {
        s.merge(w);
    }
    Mutex::new(s)
}

fn entries_of(s: &Mutex<State>) -> Vec<WireEntry> {
    s.lock().unwrap().entries()
}

/// A session run: `a` dials, `b` accepts.
async fn exchange(
    a: &Mutex<State>,
    b: &Mutex<State>,
) -> (NetResult<SessionTally>, NetResult<SessionTally>) {
    let (x, y) = duplex(1 << 20);
    let (xr, mut xw) = split(x);
    let (yr, mut yw) = split(y);
    let (mut xr, mut yr) = (xr, yr);
    tokio::join!(dial(&mut xr, &mut xw, a), accept(&mut yr, &mut yw, b))
}

#[tokio::test]
async fn fresh_pair_exchanges_everything() {
    let a = state_with(
        1,
        vec![
            put(1, b"a", b"1", 1),
            put(1, b"b", b"1", 2),
            put(1, b"c", b"1", 3),
        ],
    );
    let b = state_with(2, vec![put(3, b"x", b"1", 1), put(3, b"y", b"1", 2)]);
    let (da, ab) = exchange(&a, &b).await;
    let (da, ab) = (da.unwrap(), ab.unwrap());
    assert_eq!((da.sent, da.received), (3, 2));
    assert_eq!((ab.sent, ab.received), (2, 3));
    assert!(da.changed && ab.changed);
    assert_eq!(entries_of(&a).len(), 5);
    assert_eq!(entries_of(&a), entries_of(&b));
}

#[tokio::test]
async fn equal_states_send_nothing() {
    let items = vec![put(1, b"a", b"1", 1), put(2, b"b", b"1", 2)];
    let (a, b) = (state_with(1, items.clone()), state_with(2, items));
    let (da, ab) = exchange(&a, &b).await;
    let (da, ab) = (da.unwrap(), ab.unwrap());
    assert_eq!((da.sent, ab.sent), (0, 0));
    assert!(!da.changed && !ab.changed);
}

#[tokio::test]
async fn gap_below_max_repaired() {
    let (old, new) = (put(3, b"old", b"v", 5), put(3, b"new", b"v", 9));
    let a = state_with(1, vec![old.clone(), new.clone()]);
    let b = state_with(2, vec![new]);
    exchange(&a, &b).await.0.unwrap();
    assert!(entries_of(&b).contains(&old));
}

#[tokio::test]
async fn tombstone_overrides_live() {
    let a = state_with(1, vec![put(3, b"k", b"", 11)]);
    let b = state_with(2, vec![put(3, b"k", b"v", 10)]);
    exchange(&a, &b).await.0.unwrap();
    assert_eq!(entries_of(&b), vec![put(3, b"k", b"", 11)]);
}

#[tokio::test]
async fn bad_signature_dropped() {
    let mut bad = put(1, b"bad", b"v", 1);
    bad.value = b"x".to_vec();
    let a = state_with(1, vec![]);
    a.lock().unwrap().merge(bad);
    a.lock().unwrap().merge(put(1, b"good", b"v", 2));
    let b = state_with(2, vec![]);
    let (da, ab) = exchange(&a, &b).await;
    assert_eq!(da.unwrap().sent, 2);
    let ab = ab.unwrap();
    assert_eq!((ab.rejected, ab.received), (1, 1));
    assert_eq!(entries_of(&b), vec![put(1, b"good", b"v", 2)]);
}

#[tokio::test]
async fn own_entries_restored() {
    let a = state_with(1, vec![]);
    let b = state_with(2, vec![put(1, b"k", b"v", 100)]);
    exchange(&a, &b).await.0.unwrap();
    assert_eq!(entries_of(&a).len(), 1);
    assert!(a.lock().unwrap().next_ts_micros(5) > 100);
}

#[tokio::test]
async fn wrong_version_refused() {
    let (x, y) = duplex(1 << 16);
    let (_xr, mut xw) = split(x);
    let (mut yr, mut yw) = split(y);
    let b = state_with(2, vec![]);
    let bad = Frame::Summary {
        version: 2,
        writers: vec![],
    };
    write_framed(&mut xw, &bad, MAX_FRAME_BYTES).await.unwrap();
    let res = accept(&mut yr, &mut yw, &b).await;
    assert!(matches!(res, Err(NetError::Refused)), "{res:?}");
}

#[tokio::test]
async fn entry_limit_aborts() {
    let (x, y) = duplex(1 << 22);
    let (_xr, mut xw) = split(x);
    let (mut yr, mut yw) = split(y);
    let b = state_with(2, vec![]);
    let mut junk = put(1, &[b'k'; 600], b"v", 1);
    junk.key.truncate(513);
    let summary = Frame::Summary {
        version: PROTO_VERSION,
        writers: vec![],
    };
    write_framed(&mut xw, &summary, MAX_FRAME_BYTES)
        .await
        .unwrap();
    let pump = async {
        let batch = vec![junk; 50];
        for _ in 0..=(MAX_SESSION_ENTRIES / 50) {
            let sent = write_framed(&mut xw, &Frame::Entries(batch.clone()), MAX_FRAME_BYTES).await;
            if sent.is_err() {
                break;
            }
        }
    };
    let run = async move {
        let res = accept(&mut yr, &mut yw, &b).await;
        drop((yr, yw));
        res
    };
    let (res, ()) = tokio::join!(run, pump);
    assert!(matches!(res, Err(NetError::Refused)), "{res:?}");
}
