//! Node id (`PeerId`, an ed25519 public key) and node secret (`NodeSecret`, a seed).
//! Text and byte forms are as for the iroh-base 1.2.0 node id; there is no network here.

use data_encoding::{BASE32_NOPAD, HEXLOWER};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use zeroize::Zeroize;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdError {
    Hex,
    Base32,
    Length,
    NotOnCurve,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Hex => write!(f, "node id: invalid hex (lowercase required)"),
            IdError::Base32 => write!(f, "node id: invalid base32"),
            IdError::Length => write!(f, "node id: must be exactly 32 bytes"),
            IdError::NotOnCurve => write!(f, "node id: point is not on the ed25519 curve"),
        }
    }
}

impl std::error::Error for IdError {}

impl PeerId {
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<PeerId, IdError> {
        ed25519_dalek::VerifyingKey::from_bytes(bytes).map_err(|_| IdError::NotOnCurve)?;
        Ok(PeerId(*bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn fmt_short(&self) -> String {
        HEXLOWER.encode(&self.0)[..10].to_string()
    }

    /// Strict ed25519 signature check; any error gives `false`.
    pub fn verify(&self, msg: &[u8], sig: &[u8; 64]) -> bool {
        let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&self.0) else {
            return false;
        };
        key.verify_strict(msg, &ed25519_dalek::Signature::from_bytes(sig))
            .is_ok()
    }
}

fn decode_hex(s: &str) -> Result<[u8; 32], IdError> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(IdError::Hex);
    }
    let raw = HEXLOWER.decode(s.as_bytes()).map_err(|_| IdError::Hex)?;
    raw.try_into().map_err(|_| IdError::Length)
}

fn decode_base32(s: &str) -> Result<[u8; 32], IdError> {
    let raw = BASE32_NOPAD
        .decode(s.to_ascii_uppercase().as_bytes())
        .map_err(|_| IdError::Base32)?;
    raw.try_into().map_err(|_| IdError::Length)
}

impl FromStr for PeerId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, IdError> {
        let bytes = if s.len() == 64 {
            decode_hex(s)?
        } else {
            decode_base32(s)?
        };
        PeerId::from_bytes(&bytes)
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&HEXLOWER.encode(&self.0))
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId({self})")
    }
}

impl AsRef<[u8]> for PeerId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Serialize for PeerId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            return s.collect_str(self);
        }
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for PeerId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if d.is_human_readable() {
            let s = String::deserialize(d)?;
            return s.parse().map_err(de::Error::custom);
        }
        let bytes = <[u8; 32]>::deserialize(d)?;
        PeerId::from_bytes(&bytes).map_err(de::Error::custom)
    }
}

/// Node secret: a 32-byte ed25519 seed; wiped on drop.
#[derive(Clone)]
pub struct NodeSecret([u8; 32]);

impl NodeSecret {
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        NodeSecret(*bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        rand::fill(&mut seed);
        NodeSecret(seed)
    }

    /// ed25519 signature over `msg`.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        ed25519_dalek::SigningKey::from_bytes(&self.0)
            .sign(msg)
            .to_bytes()
    }

    pub fn peer_id(&self) -> PeerId {
        PeerId(
            ed25519_dalek::SigningKey::from_bytes(&self.0)
                .verifying_key()
                .to_bytes(),
        )
    }
}

impl Drop for NodeSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for NodeSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NodeSecret(..)")
    }
}

#[cfg(test)]
#[path = "id_tests.rs"]
mod tests;
