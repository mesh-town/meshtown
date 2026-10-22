//! Parses human input into a network key: the random part made of words, the machine form `mesh1-…`, or a pair of them
//! with a label through `--`. Encoding of the key itself and subkeys are in `network_key.rs`.

use anyhow::{anyhow, Result};
use ring::digest;

use crate::output::NET_NAME;
use crate::words::{canonical_chunk, is_tail_chunk, is_tail_word, TAIL_CHUNK_CHARS, TAIL_PARTS, TAIL_WORDS};

use super::{normalize_dashed, normalize_label, NetworkKey, CTX_TAIL_SALT, LABEL_SEPARATOR};

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
            cmd = crate::program_name::program_name()
        ));
    }
    for w in &parts[..TAIL_WORDS] {
        if !is_tail_word(w) {
            let message = format!("Looks like a typo in the word \"{w}\" — check it against what the network member sent.");
            return Err(anyhow::Error::new(BadWord { word: w.to_string(), message }));
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

/// Parsed human input: the label (for display only) and the network key.
#[derive(Debug)]
pub struct NetworkKeyInput {
    pub label: Option<String>,
    pub key: NetworkKey,
    /// The key was derived from the random part, not parsed from `mesh1-…`.
    pub from_words: bool,
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

/// Parses human input: "label--random-part", "random-part" or the machine `mesh1-…`.
pub fn parse_user_input(input: &str) -> Result<NetworkKeyInput> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "{NET_NAME} network key is not set: pass it with --key or the MESHTOWN_KEY variable. Make a new key with: {cmd} keygen <label>, for example {cmd} keygen my network",
            cmd = crate::program_name::program_name()
        ));
    }
    // The check for "--" goes before the check for "mesh1-": otherwise the label "mesh1 test" would give
    // the key "mesh1-test--…", it would go to parse and the person would get an error about base32.
    if let Some((raw_label, raw_tail)) = trimmed.split_once(LABEL_SEPARATOR) {
        let label = if normalize_dashed(raw_label).is_empty() {
            None
        } else {
            Some(normalize_label(raw_label)?)
        };
        // The right part is an encoded key (`label--mesh1-…`: this is how a key from an invite
        // lands in the settings — its words can't be restored from its bytes, and the label mustn't be lost).
        let tail_trimmed = raw_tail.trim();
        if NetworkKey::looks_like_encoded_key(tail_trimmed) {
            let key = NetworkKey::parse(tail_trimmed)?;
            return Ok(NetworkKeyInput { label, key, from_words: false });
        }
        let tail = normalize_dashed(raw_tail);
        if tail.is_empty() {
            return Err(anyhow!(
                "there is no random part of the key after \"--\". The key looks like this: label--word-word-group-of-characters. Make a new key with: {cmd} keygen <label>",
                cmd = crate::program_name::program_name()
            ));
        }
        let key = key_from_tail(&tail)?;
        return Ok(NetworkKeyInput { label, key, from_words: true });
    }
    if NetworkKey::looks_like_encoded_key(trimmed) {
        let key = NetworkKey::parse(trimmed)?;
        return Ok(NetworkKeyInput { label: None, key, from_words: false });
    }
    let tail = normalize_dashed(trimmed);
    if tail.is_empty() {
        return Err(anyhow!(
            "{NET_NAME} network key has no letters or digits left. The key looks like this: label--word-word-group-of-characters. Make a new key with: {cmd} keygen <label>",
            cmd = crate::program_name::program_name()
        ));
    }
    let parts: Vec<&str> = tail.split('-').filter(|p| !p.is_empty()).collect();
    if !looks_like_tail(&parts) {
        return Err(anyhow!(
            "the key has no random part: it looks like only the label \"{tail}\" was given. The whole key looks like this: {tail}--word-word-group-of-characters. Make a new key with: {cmd} keygen {tail}",
            cmd = crate::program_name::program_name()
        ));
    }
    let key = key_from_tail(&tail)?;
    Ok(NetworkKeyInput { label: None, key, from_words: true })
}

impl NetworkKey {
    /// Network key from human input, without the label (`parse_user_input` supplies the label).
    pub fn from_user_input(input: &str) -> Result<NetworkKey> {
        parse_user_input(input).map(|i| i.key)
    }
}
