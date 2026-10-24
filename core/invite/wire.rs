//! Frames of the `meshtown/invite/1` protocol: a request with a one-time secret and a response with a code and the network key.
//!
//! What it does: ALPN, version, response codes, building and parsing fixed-length frames.
//! What it doesn't do: doesn't touch the network and doesn't read the document; turns the storage verdict into a code.
//! Links: the server is `invite::server`, the client is `invite::redeem`; verdicts are in `invite::store`.

use super::store::IssueVerdict;
use super::InviteSecret;
use crate::members::network_key::NetworkKey;
use crate::transport::net::PeerId;

/// ALPN of the protocol that issues a key by invite.
pub const INVITE_ALPN: &[u8] = b"meshtown/invite/1";
/// Kind of the admission request and response frames: the first byte (2 for HOLD).
pub const REDEEM_FRAME_KIND: u8 = 1;
/// Request length: version and secret.
pub const REQUEST_LEN: usize = 33;
/// Kind of the owner request frame: the secret plus the newcomer's permanent id and a proof of owning it.
pub const OWNER_FRAME_KIND: u8 = 3;
/// Owner request length: kind, secret, owner id and signature.
pub const OWNER_REQUEST_LEN: usize = 129;
/// Response length: version, code and key (or zeros).
pub const REPLY_LEN: usize = 34;

/// Response code; the values are laid out in the frame and must not change.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedeemCode {
    Ok = 0,
    NotReady = 1,
    Busy = 2,
    Used = 3,
    Expired = 4,
    Unknown = 5,
    BadRequest = 6,
    Internal = 7,
    Excluded = 8,
    NotFromAdmin = 9,
}

/// All codes in value order.
const ALL: [RedeemCode; 10] = [
    RedeemCode::Ok,
    RedeemCode::NotReady,
    RedeemCode::Busy,
    RedeemCode::Used,
    RedeemCode::Expired,
    RedeemCode::Unknown,
    RedeemCode::BadRequest,
    RedeemCode::Internal,
    RedeemCode::Excluded,
    RedeemCode::NotFromAdmin,
];

/// Codes after which it makes sense for the client to retry.
const RETRYABLE: [RedeemCode; 3] = [RedeemCode::NotReady, RedeemCode::Busy, RedeemCode::Internal];

impl RedeemCode {
    /// Returns the code for a frame byte; an unknown one gives `None`.
    pub fn from_u8(b: u8) -> Option<RedeemCode> {
        ALL.iter().copied().find(|c| *c as u8 == b)
    }

    /// Whether the refusal is retryable (`NotReady`, `Busy`, `Internal`; `Excluded` is final).
    pub fn is_retryable(self) -> bool {
        RETRYABLE.contains(&self)
    }
}

/// A member's response: the network key or a refusal with a code.
#[derive(Clone, Debug)]
pub enum RedeemReply {
    Key(NetworkKey),
    Refused(RedeemCode),
}

/// Request frame: version and secret.
pub fn encode_invite_request(s: &InviteSecret) -> [u8; REQUEST_LEN] {
    let mut out = [0u8; REQUEST_LEN];
    out[0] = REDEEM_FRAME_KIND;
    out[1..].copy_from_slice(s.as_bytes());
    out
}

/// Parses a request; a foreign version gives `None`.
pub fn parse_invite_request(b: &[u8; REQUEST_LEN]) -> Option<InviteSecret> {
    if b[0] != REDEEM_FRAME_KIND {
        return None;
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&b[1..]);
    Some(InviteSecret::from_bytes(secret))
}

/// An owner request: the secret, the permanent id of the newcomer and its signature (see `invite::proof`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerRequest {
    pub secret: InviteSecret,
    pub owner: PeerId,
    pub sig: [u8; 64],
}

/// Owner request frame: kind, secret, owner id, signature.
pub fn encode_owner_request(
    s: &InviteSecret,
    owner: PeerId,
    sig: &[u8; 64],
) -> [u8; OWNER_REQUEST_LEN] {
    let mut out = [0u8; OWNER_REQUEST_LEN];
    out[0] = OWNER_FRAME_KIND;
    out[1..33].copy_from_slice(s.as_bytes());
    out[33..65].copy_from_slice(owner.as_bytes());
    out[65..].copy_from_slice(sig);
    out
}

/// Parses an owner request; a foreign kind or an owner id that is not an ed25519 key gives `None`.
pub fn parse_owner_request(b: &[u8; OWNER_REQUEST_LEN]) -> Option<OwnerRequest> {
    if b[0] != OWNER_FRAME_KIND {
        return None;
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&b[1..33]);
    let mut owner = [0u8; 32];
    owner.copy_from_slice(&b[33..65]);
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&b[65..]);
    let owner = PeerId::from_bytes(&owner).ok()?;
    Some(OwnerRequest {
        secret: InviteSecret::from_bytes(secret),
        owner,
        sig,
    })
}

/// Builds a response frame; the key is put in only for `RedeemCode::Ok`, otherwise zeros.
pub fn encode_invite_reply(code: RedeemCode, key: Option<&NetworkKey>) -> [u8; REPLY_LEN] {
    let mut out = [0u8; REPLY_LEN];
    out[0] = REDEEM_FRAME_KIND;
    out[1] = code as u8;
    let key = key.filter(|_| code == RedeemCode::Ok);
    if let Some(k) = key {
        out[2..].copy_from_slice(&k.to_bytes());
    }
    out
}

/// Parses a response; a foreign version, an unknown code and `Ok` with a zero key give `None`.
pub fn parse_invite_reply(b: &[u8; REPLY_LEN]) -> Option<RedeemReply> {
    if b[0] != REDEEM_FRAME_KIND {
        return None;
    }
    let code = RedeemCode::from_u8(b[1])?;
    if code != RedeemCode::Ok {
        return Some(RedeemReply::Refused(code));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&b[2..]);
    if key == [0u8; 32] {
        return None;
    }
    Some(RedeemReply::Key(NetworkKey::from_bytes(key)))
}

/// Turns the storage verdict into a response code; `Grant` gives `Ok` (the caller puts the key in).
pub fn code_from(v: IssueVerdict) -> RedeemCode {
    match v {
        IssueVerdict::Grant => RedeemCode::Ok,
        IssueVerdict::Used => RedeemCode::Used,
        IssueVerdict::Expired => RedeemCode::Expired,
        IssueVerdict::Unknown => RedeemCode::Unknown,
        IssueVerdict::NotReady => RedeemCode::NotReady,
        IssueVerdict::NotAdmin => RedeemCode::NotFromAdmin,
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
