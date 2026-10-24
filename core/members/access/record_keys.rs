//! Builders of the list record keys: `acl/<kind>/<hex>` and `acl-block/<hex>`. Pure functions; the
//! format of the values and the strict parse of a key are in `record.rs`.

use crate::transport::net::PeerId;

use super::record::BLOCK_PREFIX;

pub(super) const ADMIT_PREFIX: &[u8] = b"acl/admit/";
pub(super) const EXCLUDE_PREFIX: &[u8] = b"acl/exclude/";
pub(super) const FORGET_PREFIX: &[u8] = b"acl/forget/";
pub(super) const GRANT_PREFIX: &[u8] = b"acl/admin/";
pub(super) const REVOKE_PREFIX: &[u8] = b"acl/unadmin/";

pub(super) fn key_with(prefix: &[u8], subject: &PeerId) -> Vec<u8> {
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

/// Key `acl/admin/<hex>`.
pub fn grant_key(subject: &PeerId) -> Vec<u8> {
    key_with(GRANT_PREFIX, subject)
}

/// Key `acl/unadmin/<hex>`.
pub fn revoke_key(subject: &PeerId) -> Vec<u8> {
    key_with(REVOKE_PREFIX, subject)
}

/// Key `acl-block/<hex>`.
pub fn block_key(subject: &PeerId) -> Vec<u8> {
    key_with(BLOCK_PREFIX, subject)
}

pub(super) fn subject_of(key: &[u8], prefix: &[u8]) -> Option<PeerId> {
    let hex = std::str::from_utf8(key.strip_prefix(prefix)?).ok()?;
    hex.parse().ok()
}
