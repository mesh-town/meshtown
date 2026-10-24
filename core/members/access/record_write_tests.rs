//! Tests of writing and withdrawing own list records (`ensure`, `put`, `withdraw`) over an
//! in-memory replica. The format itself is covered by `record_tests.rs`.

use super::*;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::{NetError, NetResult, PeerId, Replica, ReplicaEntry};
use bytes::Bytes;
use data_encoding::HEXLOWER;
use std::sync::Arc;

// --- put ---

#[tokio::test]
async fn put_writes_key_by_kind() {
    // given: an empty replica and one value per kind (the boot marker names the writer itself)
    let r = setup();
    let other = pid(7);
    let hex = HEXLOWER.encode(other.as_bytes());
    let cases = [
        (
            AclKind::Admit,
            other,
            format!("acl/admit/{hex}").into_bytes(),
        ),
        (
            AclKind::Exclude,
            other,
            format!("acl/exclude/{hex}").into_bytes(),
        ),
        (
            AclKind::Block,
            other,
            format!("acl-block/{hex}").into_bytes(),
        ),
        (AclKind::Boot, r.me, b"acl/boot".to_vec()),
    ];
    for (kind, subject, key) in cases {
        // when: the value is put
        let value = value_of(kind, subject);
        r.put(&value).await.unwrap();
        // then: it lies under the key of its kind and parses back
        let stored = r
            .own(&key)
            .await
            .unwrap_or_else(|| panic!("nothing under the {kind:?} key"));
        let want = Parsed::Record {
            author: r.me,
            ts_micros: stored.ts_micros,
            value,
        };
        assert_eq!(parse_entry(&stored), want, "{kind:?}");
    }
}

// --- ensure ---

#[tokio::test]
async fn ensure_upgrades_my_witness_to_a_vote() {
    // given: my witness for S
    let r = setup();
    let s = pid(7);
    r.ensure(&AclValue {
        via: Via::Witness,
        ..value_of(AclKind::Admit, s)
    })
    .await
    .unwrap();
    // when: a vote of the same subject is ensured
    let changed = r
        .ensure(&AclValue {
            via: Via::None,
            ..value_of(AclKind::Admit, s)
        })
        .await
        .unwrap();
    // then: it replaced the witness
    assert!(changed);
    let stored = decode(&r.own(&admit_key(&s)).await.unwrap().value).unwrap();
    assert_eq!(stored.via, Via::None);
}

#[tokio::test]
async fn ensure_never_downgrades_a_vote_to_a_witness() {
    // given: my vote for S
    let r = setup();
    let s = pid(7);
    r.ensure(&AclValue {
        via: Via::None,
        ..value_of(AclKind::Admit, s)
    })
    .await
    .unwrap();
    // when: a witness of the same subject is ensured
    let changed = r
        .ensure(&AclValue {
            via: Via::Witness,
            ..value_of(AclKind::Admit, s)
        })
        .await
        .unwrap();
    // then: nothing is written
    assert!(!changed);
    let stored = decode(&r.own(&admit_key(&s)).await.unwrap().value).unwrap();
    assert_eq!(stored.via, Via::None);
}

#[tokio::test]
async fn ensure_keeps_equal_record() {
    // given: my witness for S
    let r = setup();
    let s = pid(7);
    let witness = AclValue {
        via: Via::Witness,
        ..value_of(AclKind::Admit, s)
    };
    r.ensure(&witness).await.unwrap();
    let stored = r.own(&admit_key(&s)).await.unwrap();
    // when: the same witness is ensured later
    let changed = r
        .ensure(&AclValue {
            made_unix: 1_800_000_000,
            ..witness
        })
        .await
        .unwrap();
    // then: nothing is written
    assert!(!changed);
    assert_eq!(r.own(&admit_key(&s)).await.unwrap(), stored);
}

#[tokio::test]
async fn rewrite_replaces_a_live_record_of_the_same_kind() {
    // given: a boot marker without a note
    let r = setup();
    let marker = AclValue {
        note: String::new(),
        ..value_of(AclKind::Boot, r.me)
    };
    r.put(&marker).await.unwrap();
    // when: it is rewritten with a note
    rewrite(
        r.rep.as_ref(),
        &AclValue {
            note: "flag".into(),
            ..marker
        },
    )
    .await
    .unwrap();
    // then: the stored marker carries the new note
    let stored = decode(&r.own(b"acl/boot").await.unwrap().value).unwrap();
    assert_eq!(stored.note, "flag");
}

#[tokio::test]
async fn ensure_keeps_first_record_of_same_subject() {
    // given: an admit of S already written
    let r = setup();
    let s = pid(7);
    let first = AclValue {
        note: "first".into(),
        ..value_of(AclKind::Admit, s)
    };
    assert!(r.ensure(&first).await.unwrap(), "a new record is a change");
    let stored = r.own(&admit_key(&s)).await.unwrap();
    // when: ensure again with another via, note and time
    let again = AclValue {
        via: Via::Witness,
        note: "second".into(),
        made_unix: 1_800_000_000,
        ..first
    };
    let changed = r.ensure(&again).await.unwrap();
    // then: nothing changed: same time, same value
    assert!(!changed, "the first record wins");
    assert_eq!(r.own(&admit_key(&s)).await, Some(stored));
}

#[tokio::test]
async fn ensure_rejects_long_note_and_foreign_boot() {
    // given: values the node itself would reject on reading
    let r = setup();
    let long = AclValue {
        note: "a".repeat(81),
        ..value_of(AclKind::Admit, pid(7))
    };
    let foreign_boot = value_of(AclKind::Boot, pid(7));
    for (name, value) in [
        ("note of 81 chars", long),
        ("boot for another subject", foreign_boot),
    ] {
        // when: ensure is asked to write them
        let result = r.ensure(&value).await;
        // then: a config error, and nothing lies under the key
        assert!(
            matches!(result, Err(NetError::Config(_))),
            "{name}: {result:?}"
        );
        assert!(
            r.own(&key_for(value.kind, &value.subject)).await.is_none(),
            "{name}"
        );
    }
    assert!(r.scan_list().await.is_empty(), "nothing written at all");
}

#[tokio::test]
async fn ensure_overwrites_tombstone_and_garbage() {
    // given: the key of S holds a tombstone, the key of T holds bytes that are no record
    let r = setup();
    let (s, t) = (pid(7), pid(8));
    r.ensure(&value_of(AclKind::Admit, s)).await.unwrap();
    withdraw(r.rep.as_ref(), AclKind::Admit, s).await.unwrap();
    r.put_garbage(&admit_key(&t)).await;
    for subject in [s, t] {
        // when: ensure writes the admit
        let value = value_of(AclKind::Admit, subject);
        let changed = r.ensure(&value).await.unwrap();
        // then: it is a change, and the stored record is good
        assert!(changed, "{subject:?}");
        let stored = r.own(&admit_key(&subject)).await.unwrap();
        let want = Parsed::Record {
            author: r.me,
            ts_micros: stored.ts_micros,
            value,
        };
        assert_eq!(parse_entry(&stored), want, "{subject:?}");
    }
}

#[tokio::test]
async fn ensure_writes_own_record_when_only_another_author_has_one() {
    // given: another author's admit of S under the same key
    let r = setup();
    let s = pid(7);
    let value = value_of(AclKind::Admit, s);
    r.rep.put_raw(ReplicaEntry {
        author: pid(8),
        key: admit_key(&s),
        value: Bytes::from(encode(&value)),
        ts_micros: 5,
    });
    // when: ensure writes our own admit of S
    let changed = r.ensure(&value).await.unwrap();
    // then: our record is written, the other author's does not count as ours
    assert!(changed);
    assert!(r.own(&admit_key(&s)).await.is_some());
}

// --- withdraw ---

#[tokio::test]
async fn withdraw_tombstones_own_record_once() {
    // given: a written admit
    let r = setup();
    let s = pid(7);
    r.ensure(&value_of(AclKind::Admit, s)).await.unwrap();
    // when: withdrawn
    let first = withdraw(r.rep.as_ref(), AclKind::Admit, s).await.unwrap();
    // then: true, and a tombstone lies under the key
    assert!(first);
    let tomb = r
        .tombstone(&admit_key(&s))
        .await
        .expect("a tombstone must lie under the key");
    // when: withdrawn again
    let second = withdraw(r.rep.as_ref(), AclKind::Admit, s).await.unwrap();
    // then: false, and the tombstone is not rewritten
    assert!(!second);
    assert_eq!(r.tombstone(&admit_key(&s)).await, Some(tomb));
}

#[tokio::test]
async fn withdraw_without_record_writes_nothing() {
    // given: an empty replica
    let r = setup();
    let s = pid(7);
    // when: a record that was never written is withdrawn
    let changed = withdraw(r.rep.as_ref(), AclKind::Exclude, s).await.unwrap();
    // then: false, and not even a tombstone appears
    assert!(!changed);
    assert!(r.own(&exclude_key(&s)).await.is_none());
    assert!(r.scan_list().await.is_empty());
}

#[tokio::test]
async fn withdraw_removes_garbage_under_own_key() {
    // given: bytes that are no record under the admit key of S
    let r = setup();
    let s = pid(7);
    r.put_garbage(&admit_key(&s)).await;
    // when: the admit of S is withdrawn
    let changed = withdraw(r.rep.as_ref(), AclKind::Admit, s).await.unwrap();
    // then: true, and the garbage is replaced by a tombstone
    assert!(changed);
    assert!(r.tombstone(&admit_key(&s)).await.is_some());
}

// --- close ---

#[tokio::test]
async fn close_for_other_is_config_error() {
    // given: a close record that names another node as its subject
    let r = setup();
    let other = value_of(AclKind::Close, pid(7));
    // when: ensure is asked to write it
    let result = r.ensure(&other).await;
    // then: a config error, and nothing is written
    assert!(matches!(result, Err(NetError::Config(_))), "{result:?}");
    assert!(r.scan_list().await.is_empty(), "nothing written at all");
    // and the close that names the own node goes under the close key
    r.ensure(&value_of(AclKind::Close, r.me)).await.unwrap();
    assert!(r.own(b"acl/close").await.is_some());
}

// --- setup and helpers ---

struct Rig {
    rep: Arc<MemReplica>,
    me: PeerId,
}

/// A node with a secret from seed 3 and its in-memory replica (the shared one).
fn setup() -> Rig {
    let node = secret(3);
    let ctx = SyncCtx {
        secret: node.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    let rep = backend.replica_mem(REPLICA_NET).unwrap();
    Rig {
        rep,
        me: node.peer_id(),
    }
}

impl Rig {
    async fn ensure(&self, v: &AclValue) -> NetResult<bool> {
        ensure(self.rep.as_ref(), v).await
    }

    async fn put(&self, v: &AclValue) -> NetResult<()> {
        put(self.rep.as_ref(), v).await
    }

    /// Our own live entry under the key.
    async fn own(&self, key: &[u8]) -> Option<ReplicaEntry> {
        self.rep.get(self.me, key).await.unwrap()
    }

    /// Every entry under `acl/`, tombstones included.
    async fn scan_list(&self) -> Vec<ReplicaEntry> {
        self.rep.scan(ACL_PREFIX).await.unwrap()
    }

    /// Our own tombstone under the key (`get` of the in-memory replica hides tombstones, `scan` shows them).
    async fn tombstone(&self, key: &[u8]) -> Option<ReplicaEntry> {
        let all = self.scan_list().await;
        all.into_iter()
            .find(|e| e.author == self.me && e.key == key && e.is_tombstone())
    }

    /// An own entry that is not a record.
    async fn put_garbage(&self, key: &[u8]) {
        self.rep
            .set_own(key, Bytes::from_static(&[0xFF, 0xFF]))
            .await
            .unwrap();
    }
}

fn value_of(kind: AclKind, subject: PeerId) -> AclValue {
    AclValue {
        kind,
        subject,
        via: Via::Invite,
        made_unix: 1_792_400_000,
        note: "laptop".to_string(),
    }
}
