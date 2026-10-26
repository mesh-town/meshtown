//! Parses human input into a network key: the words form or the raw form of a Managed key, with a label through `--`.
//! A key of an older Mesh Town is recognized and refused with its own text. Subkeys are in `network_key.rs`.

use anyhow::{anyhow, Result};
use ring::digest;

use crate::members::words::{
    canonical_chunk, is_tail_chunk, is_tail_word, TAIL_CHUNK_CHARS, TAIL_PARTS, TAIL_WORDS,
};
use crate::node::output::NET_NAME;

use super::managed::{parse_managed, NetworkKeyInput as KeyInput};
use super::{normalize_dashed, normalize_label, NetworkKey, CTX_TAIL_SALT, LABEL_SEPARATOR};

/// Prefix of the machine form of a key of an older Mesh Town.
const LEGACY_PREFIX: &str = "mesh1-";

/// Fast hash of the normalized random part: SHA-256(salt ‖ 0x0A ‖ part) → 32 bytes.
/// A slow derivation function isn't needed: the program sets the entropy, and it is 130.8 bits.
pub(super) fn derive_tail_bytes(normalized_tail: &str) -> [u8; 32] {
    let mut ctx = digest::Context::new(&digest::SHA256);
    ctx.update(CTX_TAIL_SALT.as_bytes());
    ctx.update(b"\n");
    ctx.update(normalized_tail.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.finish().as_ref());
    out
}

/// A random-part word that isn't from the built-in list. `key_from_tail` puts it into `anyhow::Error`
/// so the caller can fetch the specific word with `downcast_ref` without changing the signature
/// `key_from_tail`/`parse_user_input`.
#[derive(Debug)]
pub struct BadWord {
    pub word: String,
    pub message: String,
}

impl std::fmt::Display for BadWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BadWord {}

/// Network key from the random part: normalizes, requires two pool words and a group of 21 characters.
/// Normalization inside is mandatory: otherwise "part-" and "part" would give different keys.
/// The hash is taken of the **reduced** form (`canonical_chunk`), so
/// `o`/`0` and `i`/`l`/`1` mixed up while copying the key give the same key.
pub fn key_from_tail(tail: &str) -> Result<NetworkKey> {
    let normalized = normalize_dashed(tail);
    let parts: Vec<&str> = normalized.split('-').filter(|p| !p.is_empty()).collect();
    if parts.len() != TAIL_PARTS {
        return Err(anyhow!(
            "the random part of the key has {TAIL_WORDS} words and a group of characters, joined by hyphens ({TAIL_PARTS} parts in all), but got {}. The key is split at the first \"--\": the label is on the left, the random part on the right. Make a new key with: {cmd} keygen <label>",
            parts.len(),
            cmd = crate::cli::program_name::program_name()
        ));
    }
    for w in &parts[..TAIL_WORDS] {
        if !is_tail_word(w) {
            let message = format!("Looks like a typo in the word \"{w}\" — check it against what the network member sent.");
            return Err(anyhow::Error::new(BadWord {
                word: w.to_string(),
                message,
            }));
        }
    }
    let chunk = canonical_chunk(parts[TAIL_WORDS]);
    if !is_tail_chunk(&chunk) {
        return Err(anyhow!(
            "the last part of the key must be {TAIL_CHUNK_CHARS} characters — digits and letters except i, l, o and u — but got {} characters. The letters o, i and l don't occur in the key: they are read as the digits 0 and 1",
            chunk.chars().count()
        ));
    }
    let canonical = format!("{}-{}-{}", parts[0], parts[1], chunk);
    Ok(NetworkKey::from_bytes(derive_tail_bytes(&canonical)))
}

/// Whether a string without a label looks like the random part of a key.
/// Two signs, either is enough: the group of characters is in place (then a word may have
/// been copied with a typo) or both words are from the list (then the group may have been copied with a typo).
/// Otherwise it is a label, and it is safe to repeat it in the error text.
fn looks_like_tail(parts: &[&str]) -> bool {
    parts.len() == TAIL_PARTS
        && (is_tail_chunk(&canonical_chunk(parts[TAIL_WORDS]))
            || parts[..TAIL_WORDS].iter().all(|w| is_tail_word(w)))
}

/// Shown for a key from an older Mesh Town.
pub const LEGACY_KEY_TEXT: &str =
    "This network key is from an older Mesh Town and no longer works. Ask a network admin for a new invite.";

/// The error of a key from an older Mesh Town; its text is `LEGACY_KEY_TEXT`.
#[derive(Debug)]
pub struct LegacyKey;

impl std::fmt::Display for LegacyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(LEGACY_KEY_TEXT)
    }
}

impl std::error::Error for LegacyKey {}

/// Whether the right part of a key (after the label) is shaped like a key of an older Mesh Town:
/// a machine form starting with `mesh1-`, or three parts of the random part.
fn looks_legacy(rest: &str) -> bool {
    let machine = rest.trim().as_bytes();
    if machine.len() >= LEGACY_PREFIX.len()
        && machine[..LEGACY_PREFIX.len()].eq_ignore_ascii_case(LEGACY_PREFIX.as_bytes())
    {
        return true;
    }
    let normalized = normalize_dashed(rest);
    let parts: Vec<&str> = normalized.split('-').filter(|p| !p.is_empty()).collect();
    looks_like_tail(&parts)
}

/// The error for input that is neither a key nor a key of an older Mesh Town.
fn not_a_key(trimmed: &str) -> anyhow::Error {
    let Some((raw_label, raw_tail)) = trimmed.split_once(LABEL_SEPARATOR) else {
        return no_label_error(trimmed);
    };
    if !normalize_dashed(raw_label).is_empty() {
        if let Err(e) = normalize_label(raw_label) {
            return e;
        }
    }
    labelled_error(raw_tail)
}

fn labelled_error(raw_tail: &str) -> anyhow::Error {
    let tail = normalize_dashed(raw_tail);
    if tail.is_empty() {
        return anyhow!(
            "there is no random part of the key after \"--\". The key looks like this: label--word-word-group-of-characters. Make a new key with: {cmd} keygen <label>",
            cmd = crate::cli::program_name::program_name()
        );
    }
    key_from_tail(&tail)
        .err()
        .unwrap_or_else(|| LegacyKey.into())
}

fn no_label_error(trimmed: &str) -> anyhow::Error {
    let tail = normalize_dashed(trimmed);
    if tail.is_empty() {
        return anyhow!(
            "{NET_NAME} network key has no letters or digits left. The key looks like this: label--word-word-group-of-characters. Make a new key with: {cmd} keygen <label>",
            cmd = crate::cli::program_name::program_name()
        );
    }
    anyhow!(
        "the key has no random part: it looks like only the label \"{tail}\" was given. The whole key looks like this: {tail}--word-word-group-of-characters. Make a new key with: {cmd} keygen {tail}",
        cmd = crate::cli::program_name::program_name()
    )
}

/// The label left of the first `--`, normalized; none if it is empty.
fn split_label(raw: &str) -> Result<Option<String>> {
    if normalize_dashed(raw).is_empty() {
        return Ok(None);
    }
    normalize_label(raw).map(Some)
}

/// Reads the input as a Managed key; none if it is not shaped like one.
fn parse_managed_input(trimmed: &str) -> Option<Result<KeyInput>> {
    let (label, rest) = match trimmed.split_once(LABEL_SEPARATOR) {
        Some((raw_label, rest)) => (split_label(raw_label), rest),
        None => (Ok(None), trimmed),
    };
    let found = parse_managed(label.as_ref().ok().cloned().flatten(), rest)?;
    Some(label.and(found))
}

/// The text after the first `--`, or the whole input.
fn right_part(trimmed: &str) -> &str {
    trimmed
        .split_once(LABEL_SEPARATOR)
        .map_or(trimmed, |(_, rest)| rest)
}

fn not_set() -> anyhow::Error {
    anyhow!(
        "{NET_NAME} network key is not set: pass it with --key or the MESHTOWN_KEY variable. Make a new key with: {cmd} keygen <label>, for example {cmd} keygen my network",
        cmd = crate::cli::program_name::program_name()
    )
}

/// Parses human input: a Managed key in the words or the raw form, with or without a label. A key of an
/// older Mesh Town is refused with `LegacyKey`.
pub fn parse_user_input(input: &str) -> Result<KeyInput> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(not_set());
    }
    if let Some(found) = parse_managed_input(trimmed) {
        return found;
    }
    if looks_legacy(right_part(trimmed)) {
        return Err(LegacyKey.into());
    }
    Err(not_a_key(trimmed))
}

impl NetworkKey {
    /// Network key from human input, without the label (`parse_user_input` supplies the label).
    pub fn from_user_input(input: &str) -> Result<NetworkKey> {
        parse_user_input(input).map(|i| i.key)
    }
}

#[cfg(test)]
#[path = "input_legacy_tests.rs"]
mod legacy_tests;
