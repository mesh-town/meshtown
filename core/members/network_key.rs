//! Network key: the words form `label--word-word-group-<root>`, the raw form `label--mesh2-…`, subkeys.
//!
//! What it does: parses a key typed by a human: to the left of the first `--` is a label (for the
//! human only, it does not affect the key), to the right is a random part of two EFF-list words and a group of
//! 21 characters, from which a fast hash (SHA-256 with a constant salt) derives the base of the key, followed by
//! the node id of the network creator (`managed.rs`); derives seven subkeys from the key. A key of an older
//! Mesh Town is recognized and refused with its own text (`input.rs`).
//! What it doesn't do: it neither stores the key on disk nor prints it; there is no slow key derivation function
//! (Argon2id) any more — the entropy of the random part is known exactly and equals 130.8 bits.
//! Protocol: the rules of `normalize_dashed`, `CTX_TAIL_SALT` and the composition of the random part must not
//! change — a different result means a different network; pinned by a reference test. The derivation of a
//! Managed key and the `mesh2-` form must not change either.
//! Links: `peer_record`, `dht`, `handshake`, `node` take subkeys from `SubKeys`;
//! `main` calls `parse_user_input`; `words` provides the word list and the part checks.

use anyhow::{anyhow, Result};
use data_encoding::HEXLOWER;
use icu_normalizer::ComposingNormalizerBorrowed;
use ring::rand::{SecureRandom, SystemRandom};

use crate::node::output::NET_NAME;

mod input;
mod managed;
pub use input::{key_from_tail, parse_user_input, BadWord, LegacyKey, LEGACY_KEY_TEXT};
#[cfg(test)]
pub(crate) use managed::support::test_net;
pub use managed::{
    format_managed_key, managed_key, managed_key_with_label, parse_managed, NetworkKeyInput,
};
// Only for the reference test `derive_tail_is_deterministic_and_input_sensitive` in
// `network_key_tests.rs`: the function itself remained a detail of `input` and is not exposed.
#[cfg(test)]
use crate::members::words::is_tail_word;
#[cfg(test)]
use input::derive_tail_bytes;

const CTX_DHT_SIGNING: &str = "meshtown dht signing key v1";
const CTX_DHT_SALT: &str = "meshtown dht salt v1";
const CTX_ROSTER: &str = "meshtown roster encryption key v1";
const CTX_HANDSHAKE: &str = "meshtown handshake key v1";
const CTX_GOSSIP_TOPIC: &str = "meshtown gossip topic v1";
const CTX_DOCS_NAMESPACE: &str = "meshtown docs namespace v1";
const CTX_NETWORK_ID: &str = "meshtown network id v1";
/// Salt for deriving the key from the random part: one for the whole application (part of the protocol).
const CTX_TAIL_SALT: &str = "meshtown network key tail v1";
/// Derives the key bytes of a Managed network from its base and creator (part of the protocol).
const CTX_MANAGED: &str = "meshtown managed network v1";
/// Separator of the label and the random part in the key.
pub const LABEL_SEPARATOR: &str = "--";
/// Limit of the label length after normalization, in characters.
pub const MAX_LABEL_CHARS: usize = 64;

/// General normalization: NFKC → lower case → spaces and `_` to a hyphen → everything except letters,
/// digits and hyphen is dropped → consecutive hyphens into one → hyphens at the edges are trimmed.
/// Idempotent: a repeated call changes nothing.
pub fn normalize_dashed(input: &str) -> String {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc().normalize(input);
    let lower = nfkc.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if (c.is_whitespace() || c == '_' || c == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Normalizes the network label. The label does not affect the key — it is a caption for humans.
pub fn normalize_label(input: &str) -> Result<String> {
    let label = normalize_dashed(input);
    if label.is_empty() {
        return Err(anyhow!(
            "no letters or digits are left in the label of network {NET_NAME}: use letters of any alphabet, digits, spaces, hyphens and underscores"
        ));
    }
    let len = label.chars().count();
    if len > MAX_LABEL_CHARS {
        return Err(anyhow!(
            "the label of network {NET_NAME} is longer than {MAX_LABEL_CHARS} characters (got {len}): shorten it. The label does not affect the key — it only helps tell your own {NET_NAME} networks apart"
        ));
    }
    Ok(label)
}

/// Network key: 32 random bytes known to all network members.
#[derive(Clone)]
pub struct NetworkKey([u8; 32]);

impl std::fmt::Debug for NetworkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetworkKey({})", self.subkeys().network_id_hex())
    }
}

/// Fills the buffer with cryptographically random bytes.
pub fn fill_random(buf: &mut [u8]) -> Result<()> {
    SystemRandom::new()
        .fill(buf)
        .map_err(|_| anyhow!("the system random number generator is unavailable"))
}

impl NetworkKey {
    /// Generates a new random network key.
    pub fn generate() -> Result<NetworkKey> {
        let mut bytes = [0u8; 32];
        fill_random(&mut bytes)?;
        Ok(NetworkKey(bytes))
    }

    /// Builds a network key from ready 32 bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> NetworkKey {
        NetworkKey(bytes)
    }

    /// Returns the 32 key bytes (for the protocol that hands the key out by invite).
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Derives seven subkeys for different purposes from the network key.
    pub fn subkeys(&self) -> SubKeys {
        let dht_signing_seed = blake3::derive_key(CTX_DHT_SIGNING, &self.0);
        let dht_salt_full = blake3::derive_key(CTX_DHT_SALT, &self.0);
        let mut dht_salt = [0u8; 16];
        dht_salt.copy_from_slice(&dht_salt_full[..16]);
        let roster_key = blake3::derive_key(CTX_ROSTER, &self.0);
        let handshake_key = blake3::derive_key(CTX_HANDSHAKE, &self.0);
        let gossip_topic = blake3::derive_key(CTX_GOSSIP_TOPIC, &self.0);
        let docs_namespace_seed = blake3::derive_key(CTX_DOCS_NAMESPACE, &self.0);
        let network_id_full = blake3::derive_key(CTX_NETWORK_ID, &self.0);
        let mut network_id = [0u8; 4];
        network_id.copy_from_slice(&network_id_full[..4]);
        SubKeys {
            dht_signing_seed,
            dht_salt,
            roster_key,
            handshake_key,
            gossip_topic,
            docs_namespace_seed,
            network_id,
        }
    }
}

/// Subkeys derived from the network key.
pub struct SubKeys {
    pub dht_signing_seed: [u8; 32],
    pub dht_salt: [u8; 16],
    pub roster_key: [u8; 32],
    pub handshake_key: [u8; 32],
    pub gossip_topic: [u8; 32],
    pub docs_namespace_seed: [u8; 32],
    pub network_id: [u8; 4],
}

impl std::fmt::Debug for SubKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SubKeys(network_id={})", self.network_id_hex())
    }
}

impl SubKeys {
    /// Network identifier in hexadecimal form for output.
    pub fn network_id_hex(&self) -> String {
        HEXLOWER.encode(&self.network_id)
    }
}

#[cfg(test)]
#[path = "network_key_tests.rs"]
mod tests;
