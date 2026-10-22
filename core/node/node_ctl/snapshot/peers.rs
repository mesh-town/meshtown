//! Building the member list (`peers[]`) of the snapshot: the own row first, then others in ascending
//! order of short id. Split out of `build.rs` as a block self-contained in meaning
//! — like `route_state.rs` for the route.

use std::collections::HashSet;

use crate::transport::net::PeerId;

use crate::members::stale_peers::{self, Evidence};
use crate::node::output;
use crate::site_checks::checks::CheckView;
use crate::web_gui::PeerInfo;

use super::build::Inputs;

/// A domain is considered stale (`stale`) if its freshest record is older than this threshold.
const STALE_AFTER_SEC: u64 = 7_200;

/// Builds the member list: the own row first, then others in ascending order of short id.
pub(super) fn build_peers(i: &Inputs<'_>) -> Vec<PeerInfo> {
    let check_seen_unix = stale_peers::latest_check_unix_by_author(&i.check_views, &i.domain_names);
    let ev = Evidence {
        members: &i.members,
        telemetry: &i.telemetry,
        now: i.now_instant,
        now_unix: i.now_unix,
        check_seen_unix: &check_seen_unix,
        cache_seen_unix: &i.cached_seen,
        doc_seen_unix: &i.doc_seen,
    };
    let others = other_peer_ids(i, &ev);
    let mut peers = Vec::with_capacity(others.len() + 1);
    peers.push(self_peer_info(i));
    for id in others {
        peers.push(other_peer_info(i, &ev, id));
    }
    peers
}

/// The set of other members: the union of `members`, the keys of `telemetry.peers`, authors of check
/// records for domains from `domain_names` and the keys of `cached_seen` (the on-disk
/// address cache), without the node itself and without departed members (7 days without
/// news — `stale_peers::is_gone`; chosen exits stay); sorted ascending by
/// short id.
fn other_peer_ids(i: &Inputs<'_>, ev: &Evidence<'_>) -> Vec<PeerId> {
    let mut ids: HashSet<PeerId> = i.members.iter().copied().collect();
    ids.extend(i.telemetry.peers.keys().copied());
    for v in &i.check_views {
        if i.domain_names.iter().any(|d| d == &v.domain) {
            ids.insert(v.author);
        }
    }
    ids.extend(i.cached_seen.keys().copied());
    ids.remove(&i.id);
    ids.retain(|id| {
        i.chosen_exits.contains(id) || !stale_peers::is_gone(stale_peers::age_sec(ev, *id))
    });
    let mut others: Vec<PeerId> = ids.into_iter().collect();
    others.sort_by_key(output::short_peer_id);
    others
}

/// The snapshot's own row: always `online`, the path is not computed (`None`), the exit flag is the direct flag
/// `exit_on`.
fn self_peer_info(i: &Inputs<'_>) -> PeerInfo {
    let (self_checked, self_freshest) =
        author_stats(&i.check_views, &i.domain_names, |a| *a == i.me);
    PeerInfo {
        id: output::short_peer_id(&i.id),
        id_full: i.id.to_string(),
        name: i.self_name.map(str::to_string),
        is_self: true,
        online: true,
        path: None,
        rtt_ms: None,
        exit: Some(i.exit_on),
        country: i.countries.get(&i.id).cloned(),
        program_version: i.program_versions.get(&i.id).cloned(),
        checked: self_checked,
        stale: is_stale(self_checked, self_freshest, i.now_unix),
        last_seen_sec: 0,
        through_peer_bps: 0,
        through_us_bps: 0,
    }
}

/// The snapshot row for another member `id`: path, `rtt` and the exit flag only while online;
/// `last_seen_sec` is the age by the freshest sign of life (`stale_peers::age_sec`). The name comes from
/// `i.peer_names` (a copy of `Members::names()`), unknown — `None`.
fn other_peer_info(i: &Inputs<'_>, ev: &Evidence<'_>, id: PeerId) -> PeerInfo {
    let online = i.members.contains(&id);
    let live = i.telemetry.peers.get(&id);
    let (path, rtt_ms) = if online {
        (live.and_then(|p| p.path), live.and_then(|p| p.rtt_ms))
    } else {
        (None, None)
    };
    let (checked, freshest) = author_stats(&i.check_views, &i.domain_names, |a| {
        a.as_bytes() == id.as_bytes()
    });
    let last_seen_sec = stale_peers::age_sec(ev, id).unwrap_or(0);
    let rate = i.exit_rates.get(&id).copied().unwrap_or_default();
    PeerInfo {
        id: output::short_peer_id(&id),
        id_full: id.to_string(),
        name: i.peer_names.get(&id).cloned(),
        is_self: false,
        online,
        path,
        rtt_ms,
        exit: if online {
            i.exits.get(&id).copied()
        } else {
            None
        },
        country: i.countries.get(&id).cloned(),
        program_version: i.program_versions.get(&id).cloned(),
        checked,
        stale: is_stale(checked, freshest, i.now_unix),
        last_seen_sec,
        through_peer_bps: rate.through_peer_bps,
        through_us_bps: rate.through_us_bps,
    }
}

/// The number of domains from `list` for which the author has a record, and the time (`at`) of the freshest of them.
fn author_stats(
    views: &[CheckView],
    list: &[String],
    matches: impl Fn(&PeerId) -> bool,
) -> (u64, Option<u64>) {
    let mut checked = 0u64;
    let mut freshest: Option<u64> = None;
    for v in views {
        if !matches(&v.author) || !list.iter().any(|d| d == &v.domain) {
            continue;
        }
        if let Some(r) = &v.record {
            checked += 1;
            freshest = Some(freshest.map_or(r.at, |f| f.max(r.at)));
        }
    }
    (checked, freshest)
}

/// `stale`: there is at least one checked record and the freshest of them is older than `STALE_AFTER_SEC`.
fn is_stale(checked: u64, freshest: Option<u64>, now_unix: u64) -> bool {
    checked > 0 && freshest.is_some_and(|f| now_unix.saturating_sub(f) > STALE_AFTER_SEC)
}
