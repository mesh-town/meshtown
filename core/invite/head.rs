//! The head of a one-time invite: the one-time secret, the network mark, the label and the issue time.
//!
//! What it does: stores the secret (with the hash for the network document), checks freshness and belonging to
//! the network, assembles the key text with the label from the string.
//! What it doesn't do: doesn't know the string format and the network — those are `invite`, `invite::store`, `invite::wire`.
//! Links: the head is used by the issue store, the server, the client and the string parser.

use anyhow::{anyhow, Result};

use crate::members::network_key::{self, managed_key, managed_key_with_label, NetKind, NetworkKey};

/// How many seconds an invite is valid from the moment of issue (a day).
pub const INVITE_TTL_SECS: u64 = 86_400;

/// The secret hashing context: domain separation, the secret can't be recovered from the hash.
const CTX_INVITE_SECRET: &str = "meshtown invite secret v1";

/// The refusal text for an expired invite.
const TEXT_EXPIRED: &str =
    "The invite was issued more than a day ago and is no longer valid. Ask a \
    network member for a new one.";

/// The one-time invite secret (32 bytes); it does not get into the log or `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct InviteSecret([u8; 32]);

impl std::fmt::Debug for InviteSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InviteSecret(..)")
    }
}

impl InviteSecret {
    /// A new random secret; a random number generator error is `Err`.
    pub fn generate() -> Result<Self> {
        let mut bytes = [0u8; 32];
        network_key::fill_random(&mut bytes)?;
        Ok(InviteSecret(bytes))
    }

    /// A secret from ready 32 bytes.
    pub fn from_bytes(b: [u8; 32]) -> Self {
        InviteSecret(b)
    }

    /// The bytes of the secret.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The hash of the secret for document keys: 64 hex characters in lowercase.
    pub fn hash_hex(&self) -> String {
        hash_hex_of(&self.hash_bytes())
    }

    /// The raw hash of the secret (32 bytes): the HOLD frame carries it, the hex of it is the entry key.
    pub fn hash_bytes(&self) -> [u8; 32] {
        blake3::derive_key(CTX_INVITE_SECRET, &self.0)
    }
}

/// The hash of the secret as a document key: 64 hex characters in lowercase.
pub fn hash_hex_of(b: &[u8; 32]) -> String {
    data_encoding::HEXLOWER.encode(b)
}

/// The invite head: everything except the members' addresses.
#[derive(Clone, Debug)]
pub struct InviteHead {
    pub secret: InviteSecret,
    pub network_id: [u8; 4],
    pub label: Option<String>,
    pub made_unix: u64,
    /// The kind of the network; a Managed one carries its creator.
    pub kind: NetKind,
}

impl InviteHead {
    /// The network key a redemption reply gives: Peer — the reply itself, Managed — derived from the
    /// reply (the base) and the creator.
    pub fn reply_key(&self, reply: &NetworkKey) -> NetworkKey {
        let Some(root) = self.kind.root() else {
            return reply.clone();
        };
        managed_key(&reply.to_bytes(), &root)
    }

    /// The key text in the same form that a person enters: "label--mesh1-…" or just "mesh1-…";
    /// for a Managed network "label--mesh2-…".
    pub fn key_text(&self, reply: &NetworkKey) -> String {
        let Some(root) = self.kind.root() else {
            return network_key::key_with_label(self.label.as_deref(), reply);
        };
        managed_key_with_label(self.label.as_deref(), &reply.to_bytes(), &root)
    }
}

#[cfg(test)]
#[path = "head_tests.rs"]
mod tests;

/// The invite is expired (`now >= made_unix + TTL`) — `Err` with a text for a person.
pub fn check_fresh(head: &InviteHead, now: u64) -> Result<()> {
    let until = head.made_unix.saturating_add(INVITE_TTL_SECS);
    if now >= until {
        return Err(anyhow!(TEXT_EXPIRED));
    }
    Ok(())
}

/// Whether the `network_id` of the key in the text and of the invite is the same; an unparsable key is `Err`.
pub fn same_network(head: &InviteHead, key_text: &str) -> Result<bool> {
    let parsed = network_key::parse_user_input(key_text)?;
    Ok(parsed.key.subkeys().network_id == head.network_id)
}
