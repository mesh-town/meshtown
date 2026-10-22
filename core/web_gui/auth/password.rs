//! Hash of the page password: PBKDF2-HMAC-SHA256 from `ring`. `hash`/`verify` hold a thread
//! for ~0.1–1.5 s — call only from `spawn_blocking` under a semaphore.

use std::num::NonZeroU32;

use icu_normalizer::ComposingNormalizerBorrowed;
use ring::pbkdf2::{derive, verify as pbkdf2_verify, PBKDF2_HMAC_SHA256};

pub(crate) const PBKDF2_ITERATIONS: u32 = 600_000;
pub(crate) const MIN_PASSWORD_CHARS: usize = 8;
pub(crate) const MAX_PASSWORD_CHARS: usize = 256;
/// Cap on iterations in the file: a typo in the file does not hang the node.
pub(crate) const MAX_FILE_ITERATIONS: u32 = 10_000_000;

#[derive(Clone, PartialEq)]
pub(crate) struct PasswordRecord {
    pub(crate) iterations: NonZeroU32,
    pub(crate) salt: [u8; 16],
    pub(crate) hash: [u8; 32],
    pub(crate) set_at: u64,
}

impl std::fmt::Debug for PasswordRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordRecord")
            .field("iterations", &self.iterations)
            .field("set_at", &self.set_at)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PasswordVerdict {
    Match,
    MatchNeedsRehash,
    Mismatch,
}

/// Password in NFC: "é" from macOS (NFD) and from a phone (NFC) is one password.
fn normalized(pw: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(pw)
        .into_owned()
}

pub(crate) fn check_format(pw: &str) -> Result<(), String> {
    let n = pw.chars().count();
    if (MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&n) {
        return Ok(());
    }
    Err("password must be 8 to 256 characters".to_string())
}

pub(crate) fn hash(pw: &str) -> Result<PasswordRecord, String> {
    let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).expect("the constant is not zero");
    hash_with(pw, iterations)
}

/// Err — only from the random number generator.
pub(crate) fn hash_with(pw: &str, iterations: NonZeroU32) -> Result<PasswordRecord, String> {
    let mut salt = [0u8; 16];
    crate::members::network_key::fill_random(&mut salt).map_err(|e| e.to_string())?;
    let mut out = [0u8; 32];
    derive(
        PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        normalized(pw).as_bytes(),
        &mut out,
    );
    Ok(PasswordRecord {
        iterations,
        salt,
        hash: out,
        set_at: crate::web_gui::now_unix(),
    })
}

pub(crate) fn verify(rec: &PasswordRecord, pw: &str) -> PasswordVerdict {
    if pw.chars().count() > MAX_PASSWORD_CHARS {
        return PasswordVerdict::Mismatch;
    }
    let ok = pbkdf2_verify(
        PBKDF2_HMAC_SHA256,
        rec.iterations,
        &rec.salt,
        normalized(pw).as_bytes(),
        &rec.hash,
    )
    .is_ok();
    verdict_of(ok, rec.iterations.get())
}

fn verdict_of(matched: bool, iterations: u32) -> PasswordVerdict {
    if !matched {
        return PasswordVerdict::Mismatch;
    }
    if iterations < PBKDF2_ITERATIONS {
        return PasswordVerdict::MatchNeedsRehash;
    }
    PasswordVerdict::Match
}

#[cfg(test)]
#[path = "password_tests.rs"]
mod tests;
