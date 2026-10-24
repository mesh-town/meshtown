//! Managed networks: the kind of a network, the key that carries the creator's node id, and its two text forms.
//!
//! A Managed key is derived from a base (32 bytes, the same thing a Peer key is) and the node id of the creator:
//! a different creator gives a different network. Text forms (RFC 4648 base32 without padding, lower case):
//! - words: `label--word-word-group-<root52>`, what the page issues on creation;
//! - raw: `label--mesh2-<base ‖ root>`, what a node that joined by an invite keeps.
//!
//! Neither form is understood by the Peer parser, so a program that doesn't know Managed networks rejects them.

use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD_NOCASE;

use super::{format_key_string, key_from_tail, normalize_dashed, NetworkKey, CTX_MANAGED};
use crate::transport::net::PeerId;

/// Prefix of the raw Managed key.
const RAW_PREFIX: &str = "mesh2-";
/// Base32 length of a 32-byte node id.
const ROOT_CHARS: usize = 52;
/// Parts of a words key: two words, a group, the node id.
const WORD_PARTS: usize = 4;

/// The kind of a network: who decides about members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetKind {
    /// Members decide by vote; the key alone lets a node in.
    #[default]
    Peer,
    /// Administrators decide; `root` is the creator, an administrator for good.
    Managed { root: PeerId },
}

impl NetKind {
    /// The creator of a Managed network. The only `match` on the kind: the rest of the code asks this.
    pub fn root(&self) -> Option<PeerId> {
        match self {
            NetKind::Peer => None,
            NetKind::Managed { root } => Some(*root),
        }
    }
}

/// The 32 key bytes of a Managed network from its base and creator.
pub fn managed_key(base: &[u8; 32], root: &PeerId) -> NetworkKey {
    let mut material = [0u8; 64];
    material[..32].copy_from_slice(base);
    material[32..].copy_from_slice(root.as_bytes());
    NetworkKey::from_bytes(blake3::derive_key(CTX_MANAGED, &material))
}

fn root_text(root: &PeerId) -> String {
    data_encoding::BASE32_NOPAD
        .encode(root.as_bytes())
        .to_ascii_lowercase()
}

/// Words form of a Managed key: `label--<tail>-<root52>`.
pub fn format_managed_key(label: Option<&str>, tail: &str, root: &PeerId) -> String {
    format_key_string(label, &format!("{tail}-{}", root_text(root)))
}

/// Raw form of a Managed key: `label--mesh2-<base32 of base ‖ root>`.
pub fn managed_key_with_label(label: Option<&str>, base: &[u8; 32], root: &PeerId) -> String {
    let mut material = [0u8; 64];
    material[..32].copy_from_slice(base);
    material[32..].copy_from_slice(root.as_bytes());
    let body = data_encoding::BASE32_NOPAD
        .encode(&material)
        .to_ascii_lowercase();
    format_key_string(label, &format!("{RAW_PREFIX}{body}"))
}

/// Parsed human input: the label (for display only), the network key and what it was made of.
pub struct NetworkKeyInput {
    pub label: Option<String>,
    /// The 32 key bytes of the network.
    pub key: NetworkKey,
    /// The key was derived from the random part, not parsed from a machine form.
    pub from_words: bool,
    pub kind: NetKind,
    /// What the key is derived from; equals the key bytes in a Peer network.
    pub base: [u8; 32],
}

impl std::fmt::Debug for NetworkKeyInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkKeyInput")
            .field("label", &self.label)
            .field("key", &self.key)
            .field("from_words", &self.from_words)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl NetworkKeyInput {
    /// The secret an invite redemption answers with: the key in a Peer network, the base in a Managed one.
    pub fn reply_secret(&self) -> [u8; 32] {
        self.base
    }
}

fn has_raw_prefix(rest: &str) -> bool {
    let b = rest.as_bytes();
    b.len() >= RAW_PREFIX.len() && b[..RAW_PREFIX.len()].eq_ignore_ascii_case(RAW_PREFIX.as_bytes())
}

fn root_from(bytes: &[u8]) -> Result<PeerId> {
    let damaged = || {
        anyhow!("the network creator part of the key is damaged — copy the key in full once more")
    };
    let array: [u8; 32] = bytes.try_into().map_err(|_| damaged())?;
    PeerId::from_bytes(&array).map_err(|_| damaged())
}

fn decode(text: &str) -> Result<Vec<u8>> {
    BASE32_NOPAD_NOCASE
        .decode(text.as_bytes())
        .map_err(|_| anyhow!("the key does not decode from base32"))
}

fn finish(
    label: Option<String>,
    base: [u8; 32],
    root: PeerId,
    from_words: bool,
) -> NetworkKeyInput {
    NetworkKeyInput {
        label,
        key: managed_key(&base, &root),
        from_words,
        kind: NetKind::Managed { root },
        base,
    }
}

fn parse_raw(label: Option<String>, body: &str) -> Result<NetworkKeyInput> {
    let bytes = decode(body.trim())?;
    if bytes.len() != 64 {
        return Err(anyhow!("the key must be 64 bytes, got {}", bytes.len()));
    }
    let mut base = [0u8; 32];
    base.copy_from_slice(&bytes[..32]);
    Ok(finish(label, base, root_from(&bytes[32..])?, false))
}

fn parse_words(label: Option<String>, tail: &str, root_part: &str) -> Result<NetworkKeyInput> {
    let base = key_from_tail(tail)?.to_bytes();
    let root = root_from(&decode(root_part)?)?;
    Ok(finish(label, base, root, true))
}

/// Parses the right part of a key (after the label) as a Managed key. `None` means the string is not
/// shaped like one and the Peer parser takes it.
pub fn parse_managed(label: Option<String>, rest: &str) -> Option<Result<NetworkKeyInput>> {
    let rest = rest.trim();
    if has_raw_prefix(rest) {
        return Some(parse_raw(label, &rest[RAW_PREFIX.len()..]));
    }
    let normalized = normalize_dashed(rest);
    let parts: Vec<&str> = normalized.split('-').filter(|p| !p.is_empty()).collect();
    let shaped = parts.len() == WORD_PARTS && parts[WORD_PARTS - 1].chars().count() == ROOT_CHARS;
    shaped.then(|| {
        parse_words(
            label,
            &parts[..WORD_PARTS - 1].join("-"),
            parts[WORD_PARTS - 1],
        )
    })
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "managed_forms_tests.rs"]
mod forms_tests;

#[cfg(test)]
#[path = "managed_support.rs"]
mod support;
