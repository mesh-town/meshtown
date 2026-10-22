//! The random part of a network key: the built-in EFF word list, two words and a group of characters.
//!
//! What it does: stores the built-in EFF list of 7,776 words, carves a pool out of it (7,772 words —
//! without four words with a hyphen inside, otherwise joining with hyphens is ambiguous), issues a random
//! key part "word-word-group of 21 characters" (130.8 bits) and checks a random part
//! retyped by a person: the words against the pool, the group against the alphabet.
//! What it doesn't do: doesn't derive the key and doesn't parse the label — that is `network_key`; doesn't print the key.
//! Protocol: the word list, the group alphabet, the rule "two words plus 21 characters" and the mapping
//! of ambiguous characters are part of the key format; changing them changes which keys the program accepts.
//! Links: `network_key` calls `is_tail_word`, `canonical_chunk` and `is_tail_chunk`,
//! `main` — `generate_tail`;
//! randomness comes from `network_key::fill_random`.
//! Also: the default node name (`default_node_name`) and the check of a name set by a person
//! (`trim_and_validate_node_name`).
//!
//! Word list: EFF Large Wordlist for Passphrases, © Electronic Frontier Foundation,
//! CC BY 3.0 US, https://www.eff.org/dice — see `core/wordlist/NOTICE.md`.

use std::collections::HashSet;
use std::sync::OnceLock;

use anyhow::{anyhow, Result};

use crate::members::network_key::fill_random;

/// The built-in EFF list as is: lines "number<TAB>word".
const EFF_LARGE_WORDLIST: &str = include_str!("wordlist/eff_large_wordlist.txt");
/// The SHA-256 of the list file, recorded at download on 2026-09-22.
#[cfg(test)]
const WORDLIST_SHA256: &str = "addd35536511597a02fa0a9ff1e5284677b8883b83e986e43f15a3db996b903e";

/// The number of words in the list file.
pub const WORDLIST_LEN: usize = 7_776;
/// The number of words in the pool: the list without words containing a hyphen (see `tail_pool_vec`).
pub const TAIL_POOL_LEN: usize = 7_772;
/// How many words are in the random part of the key.
pub const TAIL_WORDS: usize = 2;
/// How many characters are in the group of the random part of the key (21 × 5 bits = 105 bits).
pub const TAIL_CHUNK_CHARS: usize = 21;
/// How many parts are in the random part of the key: two words and a group of characters.
pub const TAIL_PARTS: usize = TAIL_WORDS + 1;
/// The group alphabet: Crockford base32 — digits and letters without `i`, `l`, `o`, `u`.
/// The letters are excluded precisely because they are confused with digits; ambiguous characters are mapped by
/// `canonical_chunk`.
pub const TAIL_ALPHABET: &str = "0123456789abcdefghjkmnpqrstvwxyz";
/// The bound of the unbiased word draw: 7,772 × 8 = 62,176 ≤ 65,536.
const UNBIASED_LIMIT: u16 = 62_176;

fn wordlist() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        EFF_LARGE_WORDLIST
            .lines()
            .filter_map(|line| line.split('\t').nth(1))
            .map(str::trim)
            .collect()
    })
}

/// The word pool in list order: words without a hyphen inside — so that splitting the random part
/// by hyphens is unambiguous (`drop-down` would give two tokens, and `drop` is not in the list).
fn tail_pool_vec() -> &'static [&'static str] {
    static V: OnceLock<Vec<&'static str>> = OnceLock::new();
    V.get_or_init(|| {
        wordlist()
            .iter()
            .copied()
            .filter(|w| !w.contains('-'))
            .collect()
    })
}

/// The same pool for lookup by word.
fn tail_pool() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| tail_pool_vec().iter().copied().collect())
}

/// Whether the word is in the pool.
pub fn is_tail_word(word: &str) -> bool {
    tail_pool().contains(word)
}

/// Maps the ambiguous characters of the group to digits by the Crockford rule: `o` → `0`, `i` and `l` → `1`.
/// Needed so that a key retyped from a screen with a confused letter opens the same network.
/// The input is already lowercase after `normalize_dashed`.
pub fn canonical_chunk(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'o' => '0',
            'i' | 'l' => '1',
            other => other,
        })
        .collect()
}

/// Whether the string looks like the character group of the random part: exactly 21 characters of the alphabet.
/// Expects an already mapped string — ambiguous characters are removed by `canonical_chunk`.
pub fn is_tail_chunk(s: &str) -> bool {
    s.chars().count() == TAIL_CHUNK_CHARS && s.chars().all(|c| TAIL_ALPHABET.contains(c))
}

/// A random pool word number 0..7772 without modulo bias (values ≥ 62,176 are rejected).
fn random_word_index() -> Result<usize> {
    loop {
        let mut buf = [0u8; 2];
        fill_random(&mut buf)?;
        let v = u16::from_le_bytes(buf);
        if v < UNBIASED_LIMIT {
            return Ok(usize::from(v) % TAIL_POOL_LEN);
        }
    }
}

/// A random group character. No rejection is needed: 256 mod 32 = 0, the byte's remainder is evenly distributed.
fn random_chunk_char() -> Result<char> {
    let mut b = [0u8; 1];
    fill_random(&mut b)?;
    let alphabet = TAIL_ALPHABET.as_bytes();
    Ok(alphabet[b[0] as usize % alphabet.len()] as char)
}

/// The random part of a key: two different pool words and a group of 21 characters (130.8 bits).
pub fn generate_tail() -> Result<String> {
    let words = tail_pool_vec();
    if words.len() != TAIL_POOL_LEN {
        return Err(anyhow!(
            "the built-in word list is damaged: {} words in the pool instead of {}",
            words.len(),
            TAIL_POOL_LEN
        ));
    }
    if TAIL_ALPHABET.chars().count() != 32 {
        return Err(anyhow!(
            "the character group alphabet must have 32 characters"
        ));
    }
    let mut out = String::new();
    for i in distinct_word_indices(TAIL_WORDS)? {
        out.push_str(words[i]);
        out.push('-');
    }
    for _ in 0..TAIL_CHUNK_CHARS {
        out.push(random_chunk_char()?);
    }
    Ok(out)
}

/// `n` distinct random pool indices.
fn distinct_word_indices(n: usize) -> Result<Vec<usize>> {
    let mut picked: Vec<usize> = Vec::with_capacity(n);
    while picked.len() < n {
        let i = random_word_index()?;
        if !picked.contains(&i) {
            picked.push(i);
        }
    }
    Ok(picked)
}

/// The context for deriving a node name from the key (`blake3` in `new_derive_key` mode), separate from
/// the `network_key` contexts.
const NAME_CONTEXT: &str = "meshtown node name v1";

/// How many pool words go into the default node name.
pub const NAME_WORDS: usize = 2;
/// The length limit of a node name set by a person in the settings file, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// The length limit of a node name in bytes — for outside consumers (the handshake frame, an invite);
/// `trim_and_validate_node_name` itself doesn't count bytes (see its description).
pub const MAX_NAME_BYTES: usize = MAX_NAME_CHARS * 4; // 256

/// The default node name derived from the node's public key: `<word>-<word>` (like
/// `finch-unwary`). A pure function with no state and no errors: the 32-byte key is the only
/// input, the result is deterministic from it (the same key on any computer gives the same
/// name), so there is nothing to store here and no error to report.
pub fn default_node_name(id: &[u8; 32]) -> String {
    let words = tail_pool_vec();

    let mut hasher = blake3::Hasher::new_derive_key(NAME_CONTEXT);
    hasher.update(id);
    let mut reader = hasher.finalize_xof();

    // Words: we read byte pairs from a continuous stream until we gather NAME_WORDS distinct
    // pool indices. A rejection (a value ≥ UNBIASED_LIMIT or an index already picked) is not
    // an error — just the next byte pair from the same stream.
    let mut picked: Vec<usize> = Vec::with_capacity(NAME_WORDS);
    while picked.len() < NAME_WORDS {
        let mut buf = [0u8; 2];
        reader.fill(&mut buf);
        let v = u16::from_le_bytes(buf);
        if v >= UNBIASED_LIMIT {
            continue;
        }
        let idx = usize::from(v) % TAIL_POOL_LEN;
        if picked.contains(&idx) {
            continue;
        }
        picked.push(idx);
    }

    format!("{}-{}", words[picked[0]], words[picked[1]])
}

/// A random node name: two distinct pool words joined by a hyphen (`finch-unwary`), unbiased — the same
/// principle as [`generate_tail`]. Unlike [`default_node_name`] it is not tied to the node's
/// key: used when the settings file needs a new random name (a new node, a save
/// error, an empty name from the page). The only error is the system random generator failing.
pub fn random_node_name() -> Result<String> {
    let words = tail_pool_vec();
    let picked = distinct_word_indices(NAME_WORDS)?;
    Ok(format!("{}-{}", words[picked[0]], words[picked[1]]))
}

/// Validates a node name set by a person in the settings file: trimming whitespace at the edges, the length
/// in characters (not bytes) at most `MAX_NAME_CHARS`, no control characters and no U+2028/U+2029
/// (line/paragraph separators that would forge a log line). `Ok` returns the trimmed
/// slice of the input (without allocating a new string — `trim()` only narrows the `&str` bounds); `Err` is the text
/// of the refusal for a person. By length it is never stricter than the decision the config module already made: it
/// counts characters before trimming, here the limit applies after.
pub fn trim_and_validate_node_name(s: &str) -> Result<&str, &'static str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("name is empty");
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err("name is longer than 64 characters");
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
    {
        return Err("name contains a control character or a line break");
    }
    Ok(trimmed)
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;
