//! Format of membership list records: value bytes, keys and parsing of `acl/...` and `acl-block/...`
//! entries. Pure functions; writing lives in `record_io.rs`, the tally input builder in `record_input.rs`.
//!
//! Value layout, version 1: `[ACL_VALUE_VERSION] ++ postcard(Body)` with the fields of `Body` frozen in
//! order. Compatibility: there are no tail bytes at `v = 1` — extra bytes make the record invalid; a changed
//! body means version 2. New values of `kind` may be added at version 1 (an old node ignores an unknown kind).

use serde::{Deserialize, Serialize};

use crate::transport::net::{PeerId, ReplicaEntry};

pub use super::record_input::{tally_input, tally_input_under};
pub use super::record_io::{ensure, put, rewrite, withdraw};

/// Version byte of the value layout.
pub const ACL_VALUE_VERSION: u8 = 1;
/// Longest note, in characters.
pub const MAX_NOTE_CHARS: usize = 80;
/// Prefix of the shared list records.
pub const ACL_PREFIX: &[u8] = b"acl/";
/// Prefix of local blocks (own replica only).
pub const BLOCK_PREFIX: &[u8] = b"acl-block/";
/// Key of the boot marker.
pub const BOOT_KEY: &[u8] = b"acl/boot";
/// Note of the boot marker of a node that counts admissions by one confirmation.
pub const BOOT_NOTE_ADMISSION: &str = "admission-v2";
const ADMIT_PREFIX: &[u8] = b"acl/admit/";
const EXCLUDE_PREFIX: &[u8] = b"acl/exclude/";
const FORGET_PREFIX: &[u8] = b"acl/forget/";

/// What a record says.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AclKind {
    Admit = 1,
    Exclude = 2,
    Block = 3,
    Boot = 4,
    Forget = 5,
}

impl AclKind {
    pub fn from_u8(b: u8) -> Option<AclKind> {
        [
            AclKind::Admit,
            AclKind::Exclude,
            AclKind::Block,
            AclKind::Boot,
            AclKind::Forget,
        ]
        .into_iter()
        .find(|k| *k as u8 == b)
    }
}

/// Where an admission came from.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Via {
    None = 0,
    Invite = 1,
    /// No longer written; records carrying it are still read and counted.
    Migration = 2,
    Witness = 3,
}

impl Via {
    pub fn from_u8(b: u8) -> Option<Via> {
        [Via::None, Via::Invite, Via::Migration, Via::Witness]
            .into_iter()
            .find(|v| *v as u8 == b)
    }
}

/// A decoded record value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AclValue {
    pub kind: AclKind,
    pub subject: PeerId,
    pub via: Via,
    pub made_unix: u64,
    pub note: String,
}

#[derive(Serialize, Deserialize)]
struct Body {
    kind: u8,
    subject: [u8; 32],
    via: u8,
    made_unix: u64,
    note: String,
}

/// Why an entry is not counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ignored {
    ForeignKey,
    Empty,
    BadVersion,
    Malformed,
    TrailingBytes,
    UnknownKind,
    UnknownVia,
    BadSubject,
    NoteTooLong,
    KeyMismatch,
}

impl Ignored {
    /// A short English word for the log.
    pub fn reason(self) -> &'static str {
        match self {
            Ignored::ForeignKey => "foreign-key",
            Ignored::Empty => "empty",
            Ignored::BadVersion => "bad-version",
            Ignored::Malformed => "malformed",
            Ignored::TrailingBytes => "trailing-bytes",
            Ignored::UnknownKind => "unknown-kind",
            Ignored::UnknownVia => "unknown-via",
            Ignored::BadSubject => "bad-subject",
            Ignored::NoteTooLong => "note-too-long",
            Ignored::KeyMismatch => "key-mismatch",
        }
    }
}

/// A replica entry classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    Record {
        author: PeerId,
        ts_micros: u64,
        value: AclValue,
    },
    Withdrawn {
        author: PeerId,
        key: Vec<u8>,
    },
    Ignored(Ignored),
}

fn key_with(prefix: &[u8], subject: &PeerId) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend_from_slice(subject.to_string().as_bytes());
    key
}

/// Key `acl/admit/<hex>`.
pub fn admit_key(subject: &PeerId) -> Vec<u8> {
    key_with(ADMIT_PREFIX, subject)
}

/// Key `acl/exclude/<hex>`.
pub fn exclude_key(subject: &PeerId) -> Vec<u8> {
    key_with(EXCLUDE_PREFIX, subject)
}

/// Key `acl/forget/<hex>`.
pub fn forget_key(subject: &PeerId) -> Vec<u8> {
    key_with(FORGET_PREFIX, subject)
}

/// Key `acl-block/<hex>`.
pub fn block_key(subject: &PeerId) -> Vec<u8> {
    key_with(BLOCK_PREFIX, subject)
}

/// Key by kind; `Boot` ignores the subject.
pub fn key_for(kind: AclKind, subject: &PeerId) -> Vec<u8> {
    match kind {
        AclKind::Admit => admit_key(subject),
        AclKind::Exclude => exclude_key(subject),
        AclKind::Forget => forget_key(subject),
        AclKind::Block => block_key(subject),
        AclKind::Boot => BOOT_KEY.to_vec(),
    }
}

fn subject_of(key: &[u8], prefix: &[u8]) -> Option<PeerId> {
    let hex = std::str::from_utf8(key.strip_prefix(prefix)?).ok()?;
    hex.parse().ok()
}

/// Strict key parse: the exact form `key_for` produces, nothing else.
pub(super) fn parse_key(key: &[u8]) -> Option<(AclKind, Option<PeerId>)> {
    if key == BOOT_KEY {
        return Some((AclKind::Boot, None));
    }
    let admit = subject_of(key, ADMIT_PREFIX).map(|s| (AclKind::Admit, s));
    let exclude = || subject_of(key, EXCLUDE_PREFIX).map(|s| (AclKind::Exclude, s));
    let forget = || subject_of(key, FORGET_PREFIX).map(|s| (AclKind::Forget, s));
    let block = || subject_of(key, BLOCK_PREFIX).map(|s| (AclKind::Block, s));
    let (kind, subject) = admit.or_else(exclude).or_else(forget).or_else(block)?;
    (key_for(kind, &subject) == key).then_some((kind, Some(subject)))
}

/// Value bytes; no checks (tests build invalid values too).
pub fn encode(v: &AclValue) -> Vec<u8> {
    let body = Body {
        kind: v.kind as u8,
        subject: *v.subject.as_bytes(),
        via: v.via as u8,
        made_unix: v.made_unix,
        note: v.note.clone(),
    };
    let mut out = vec![ACL_VALUE_VERSION];
    out.extend(postcard::to_stdvec(&body).expect("Body has no serialization errors"));
    out
}

/// Parses the value; the first failure in the documented order.
pub fn decode(bytes: &[u8]) -> Result<AclValue, Ignored> {
    let (version, rest) = bytes.split_first().ok_or(Ignored::Empty)?;
    if *version != ACL_VALUE_VERSION {
        return Err(Ignored::BadVersion);
    }
    let (body, tail) = postcard::take_from_bytes::<Body>(rest).map_err(|_| Ignored::Malformed)?;
    if !tail.is_empty() {
        return Err(Ignored::TrailingBytes);
    }
    from_body(body)
}

fn from_body(body: Body) -> Result<AclValue, Ignored> {
    let kind = AclKind::from_u8(body.kind).ok_or(Ignored::UnknownKind)?;
    let via = Via::from_u8(body.via).ok_or(Ignored::UnknownVia)?;
    let subject = PeerId::from_bytes(&body.subject).map_err(|_| Ignored::BadSubject)?;
    if body.note.chars().count() > MAX_NOTE_CHARS {
        return Err(Ignored::NoteTooLong);
    }
    Ok(AclValue {
        kind,
        subject,
        via,
        made_unix: body.made_unix,
        note: body.note,
    })
}

fn matches_key(kind: AclKind, key_subject: Option<PeerId>, v: &AclValue, author: PeerId) -> bool {
    match (kind, key_subject) {
        (AclKind::Boot, _) => v.kind == kind && v.subject == author,
        (_, Some(s)) => v.kind == kind && v.subject == s,
        (_, None) => false,
    }
}

/// Classifies a replica entry. Does not check the author or the replica.
pub fn parse_entry(e: &ReplicaEntry) -> Parsed {
    let Some((kind, key_subject)) = parse_key(&e.key) else {
        return Parsed::Ignored(Ignored::ForeignKey);
    };
    if e.is_tombstone() {
        return Parsed::Withdrawn {
            author: e.author,
            key: e.key.clone(),
        };
    }
    match decode(&e.value) {
        Err(why) => Parsed::Ignored(why),
        Ok(v) if !matches_key(kind, key_subject, &v, e.author) => {
            Parsed::Ignored(Ignored::KeyMismatch)
        }
        Ok(value) => Parsed::Record {
            author: e.author,
            ts_micros: e.ts_micros,
            value,
        },
    }
}

#[cfg(test)]
#[path = "record_support_tests.rs"]
pub(super) mod support_tests;

#[cfg(test)]
#[path = "record_format_tests.rs"]
mod format_tests;

#[cfg(test)]
#[path = "record_ignore_tests.rs"]
mod ignore_tests;

#[cfg(test)]
#[path = "record_entry_tests.rs"]
mod entry_tests;

#[cfg(test)]
#[path = "record_write_tests.rs"]
mod write_tests;

#[cfg(test)]
#[path = "record_forget_tests.rs"]
mod forget_tests;
