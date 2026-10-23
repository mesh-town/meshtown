//! The forget part of the member list: the vote circle of a row, the "removed me" flag, the age of a row.
//!
//! What it does: reads the forget circles of the verdicts and the live state and fills the matching fields of
//! `GET /api/members`.
//! What it doesn't do: no decisions and no replica reads; the circles come from the ledger.

use std::collections::BTreeMap;

use crate::members::membership::Members;
use crate::node::output::short_peer_id;
use crate::transport::net::PeerId;
use crate::web_gui::{ForgetInfo, MembersReply};

use super::view::{Deny, Refusal, Side, Verdicts};

/// The circle of votes on `p`, when there is one.
pub(super) fn forget_info(v: &Verdicts, p: &PeerId) -> Option<ForgetInfo> {
    let round = v.ledger.forgets.get(p)?;
    let proposed_by = short_peer_id(round.voters.first()?);
    Some(ForgetInfo {
        votes: round.voters.len(),
        need: v.ledger.forget_need,
        proposed_by,
        voters: round.voters.iter().map(short_peer_id).collect(),
        mine: round.voters.contains(&v.me),
        at_unix: round.at_unix,
    })
}

/// True when a neighbour turned this node away as removed from its member list.
fn refused_as_removed(refusals: &[(PeerId, Refusal)]) -> bool {
    refusals
        .iter()
        .any(|(_, r)| r.side == Side::Them && r.deny == Deny::Forgotten)
}

/// True when this node is removed in its own view, or neighbours say so while it is connected to nobody.
pub(super) fn me_removed(v: &Verdicts, members: &Members, refusals: &[(PeerId, Refusal)]) -> bool {
    v.ledger.is_forgotten(&v.me) || (refused_as_removed(refusals) && members.ids().is_empty())
}

/// Sets `last_seen_sec` of each row from the map of ages (seconds since the last sign of life) by id.
pub fn stamp_last_seen(reply: &mut MembersReply, seen: &BTreeMap<PeerId, u64>) {
    for row in &mut reply.members {
        row.last_seen_sec = row
            .id_full
            .parse::<PeerId>()
            .ok()
            .and_then(|p| seen.get(&p).copied());
    }
}
