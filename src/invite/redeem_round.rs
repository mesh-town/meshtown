//! Pure decisions of the invite client: which frame is sent and what the members' answers of a round mean.
//!
//! What it does: the request mode (owner frame, or the older frame after a fallback), the two prepared frames,
//! the outcome of a round by the answers and the final refusals.
//! What it doesn't do: no network, no waiting (`invite::redeem`).
//! Links: used by `invite::redeem`; the codes are in `invite::wire`.

use super::proof::sign_owner;
use super::redeem::RedeemError;
use super::wire::{
    encode_invite_request, encode_owner_request, RedeemCode, RedeemReply, OWNER_REQUEST_LEN, REQUEST_LEN,
};
use super::InviteHead;
use crate::net::NodeSecret;
use crate::network_key::NetworkKey;

/// Which request the client sends: the owner frame first; the older one once members turn out to be old.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Owner,
    Legacy,
}

/// Both request frames, built once per redemption.
pub(super) struct Frames {
    owner: [u8; OWNER_REQUEST_LEN],
    legacy: [u8; REQUEST_LEN],
}

impl Frames {
    /// The owner frame is signed for the one-time transport `transport_id`.
    pub(super) fn new(head: &InviteHead, transport_id: crate::net::PeerId, owner: &NodeSecret) -> Self {
        let sig = sign_owner(owner, &head.secret, transport_id);
        Frames {
            owner: encode_owner_request(&head.secret, owner.peer_id(), &sig),
            legacy: encode_invite_request(&head.secret),
        }
    }

    pub(super) fn of(&self, mode: Mode) -> &[u8] {
        match mode {
            Mode::Owner => &self.owner,
            Mode::Legacy => &self.legacy,
        }
    }
}

/// The outcome of a polling round.
#[derive(Debug)]
pub(super) enum Outcome {
    Key(NetworkKey),
    Refused(RedeemError),
    AllUnknown,
    /// Everyone asked refused the owner frame as a bad request: they run an older version.
    Legacy,
    Retry,
}

impl Outcome {
    pub(super) fn decisive(&self) -> bool {
        matches!(self, Outcome::Key(_) | Outcome::Refused(_))
    }
}

/// The outcome of a round by the members' answers (a pure function).
pub(super) fn round_outcome(answers: &[Option<RedeemReply>], network_id: [u8; 4], mode: Mode) -> Outcome {
    let replies: Vec<&RedeemReply> = answers.iter().flatten().collect();
    let keys = replies.iter().filter_map(|r| key_of(r));
    if let Some(own) = keys.clone().find(|k| k.subkeys().network_id == network_id) {
        return Outcome::Key(own.clone());
    }
    if keys.count() > 0 {
        return Outcome::Refused(RedeemError::WrongNetwork);
    }
    let codes: Vec<RedeemCode> = replies.iter().filter_map(|r| code_of(r)).collect();
    if let Some(why) = codes.iter().find_map(|c| final_refusal(*c, mode)) {
        return Outcome::Refused(why);
    }
    undecided(&codes, answers.len())
}

fn key_of(r: &RedeemReply) -> Option<&NetworkKey> {
    let RedeemReply::Key(k) = r else { return None };
    Some(k)
}

fn code_of(r: &RedeemReply) -> Option<RedeemCode> {
    let RedeemReply::Refused(c) = r else { return None };
    Some(*c)
}

/// The final refusal by code; `None` — retry or count as "unknown". A bad request is final only in the
/// older mode: in the owner mode it means the member is old.
pub(super) fn final_refusal(c: RedeemCode, mode: Mode) -> Option<RedeemError> {
    match (c, mode) {
        (RedeemCode::Used, _) => Some(RedeemError::Used),
        (RedeemCode::Expired, _) => Some(RedeemError::Expired),
        (RedeemCode::Excluded, _) => Some(RedeemError::Excluded),
        (RedeemCode::BadRequest, Mode::Legacy) => Some(RedeemError::Unknown),
        _ => None,
    }
}

/// Only when all those asked answered does the round say anything but "retry": all `BadRequest` is a fallback,
/// every answer `Unknown` or `BadRequest` with at least one `Unknown` is "unknown". A silent member might
/// know the invite, so a round with silence is a retry.
fn undecided(codes: &[RedeemCode], asked: usize) -> Outcome {
    let all_answered = !codes.is_empty() && codes.len() == asked;
    let all_bad = all_answered && codes.iter().all(|c| *c == RedeemCode::BadRequest);
    let none_known = all_answered && codes.iter().all(|c| matches!(c, RedeemCode::Unknown | RedeemCode::BadRequest));
    match (all_bad, none_known) {
        (true, _) => Outcome::Legacy,
        (false, true) => Outcome::AllUnknown,
        (false, false) => Outcome::Retry,
    }
}
