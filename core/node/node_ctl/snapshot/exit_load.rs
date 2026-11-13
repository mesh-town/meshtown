//! The overload fields of the snapshot: what a member's record says, the own live state and the client's local sign.
//!
//! What it does: decides when another exit's record counts (the author is online with the exit flag on, the stamp
//! is within `RECORD_TTL` of the reader's clock, the value was readable) and lays the figures out for the page.
//! What it doesn't do: it reads nothing — `collect` hands over the record map, the live meter and the pool counts.

use std::collections::HashMap;

use crate::exit::load::{Cause, ExitLoad};
use crate::exit::LoadView;
use crate::transport::net::PeerId;
use crate::web_gui::{ExitLoadInfo, ExitLoadMember, TurnedAway};

use super::build::Inputs;

/// What `collect` hands over for the overload fields.
#[derive(Default)]
pub(super) struct LoadInputs {
    /// Overload records of the exits from the document.
    pub records: HashMap<PeerId, ExitLoad>,
    /// The live load of this process's exit; `None` while the exit of this network is off.
    pub own: Option<LoadView>,
    /// Chosen exits that refuse this client now: id → (refused, attempts) over two minutes.
    pub turned_away: HashMap<PeerId, (u64, u64)>,
}

/// How far a record's stamp may lie from the reader's clock, in either direction, microseconds (25 minutes).
pub(super) const RECORD_TTL_MICROS: u64 = 25 * 60 * 1_000_000;

fn word(cause: Cause) -> String {
    match cause {
        Cause::Known(reason) => reason.as_str().to_string(),
        Cause::Other => "other".to_string(),
    }
}

fn is_fresh(record: &ExitLoad, now_unix: u64) -> bool {
    let now = now_unix.saturating_mul(1_000_000);
    now.abs_diff(record.ts_micros) <= RECORD_TTL_MICROS
}

/// The reason in another member's record, if the record counts.
pub(super) fn record_reason(i: &Inputs<'_>, id: &PeerId) -> Option<String> {
    let online_exit = i.members.contains(id) && i.exits.get(id) == Some(&true);
    let record = i.load.records.get(id).filter(|_| online_exit)?;
    is_fresh(record, i.now_unix).then(|| word(record.cause))
}

/// The own row's `overloaded`: the live state, only while the exit of this network is on.
pub(super) fn own_reason(i: &Inputs<'_>) -> Option<String> {
    let load = i.load.own.as_ref().filter(|_| i.exit_on)?;
    load.counts.overloaded.map(|r| r.as_str().to_string())
}

/// `route.exits[].overloaded` for any chosen exit, the own node included.
pub(super) fn chosen_reason(i: &Inputs<'_>, id: &PeerId) -> Option<String> {
    match *id == i.id {
        true => own_reason(i),
        false => record_reason(i, id),
    }
}

/// The client's local sign for one exit; never for the own node.
pub(super) fn turned_away_of(i: &Inputs<'_>, id: &PeerId) -> Option<TurnedAway> {
    let (refused, attempts) = i
        .load
        .turned_away
        .get(id)
        .copied()
        .filter(|_| *id != i.id)?;
    Some(TurnedAway { refused, attempts })
}

/// `exit.load`: `None` while the exit of this network is off.
pub(super) fn exit_load_info(i: &Inputs<'_>) -> Option<ExitLoadInfo> {
    let view = i.load.own.as_ref().filter(|_| i.exit_on)?;
    let members = member_rows(i, view);
    let listed: u64 = members.iter().map(|m| m.refused).sum();
    Some(ExitLoadInfo {
        streams: view.streams,
        limit: view.limit,
        limit_auto: view.limit_auto,
        overloaded: view.counts.overloaded.map(|r| r.as_str().to_string()),
        admitted: view.counts.admitted,
        refused: view.counts.refused,
        elsewhere_refused: view.counts.refused.saturating_sub(listed),
        members,
    })
}

fn member_rows(i: &Inputs<'_>, view: &LoadView) -> Vec<ExitLoadMember> {
    view.counts
        .members
        .iter()
        .filter(|m| i.members.contains(&m.peer))
        .map(|m| ExitLoadMember {
            id_full: m.peer.to_string(),
            admitted: m.admitted,
            refused: m.refused,
        })
        .collect()
}

/// Local signs of the chosen exits, read from the pool of this network.
pub(super) fn turned_away_map(
    chosen: &[PeerId],
    mut read: impl FnMut(PeerId) -> Option<(u64, u64)>,
) -> HashMap<PeerId, (u64, u64)> {
    chosen
        .iter()
        .filter_map(|id| read(*id).map(|v| (*id, v)))
        .collect()
}
