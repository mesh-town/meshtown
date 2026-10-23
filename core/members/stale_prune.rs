//! The hourly sweep of departed members from the address cache and from the name store on disk.
//!
//! What it does: once per `PRUNE_EVERY` (the first time `FIRST_PRUNE_AFTER` after start) finds
//! in `peers-cache.json` and `peer-names.json` those not seen for longer than `GONE_AFTER_SECS`, and
//! removes them from memory and from disk. Cuts only one who is "gone" on two ticks in a row: after
//! a laptop's long sleep the first tick may come before the document syncs.
//! What it doesn't do: doesn't touch itself, network members and chosen exits; doesn't clean `Members`
//! and telemetry; cuts nothing on any read failure.
//! Links: `node::event_loop::run_node_loop` calls `prune_tick`; the age and the "gone" rule are in
//! `stale_peers`; `PeersCache::forget`, `PeerNames::forget` change memory, saving is here.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::time::{Duration, Instant};

use crate::transport::net::PeerId;

use crate::members::membership::Members;
use crate::members::peer_names::PeerNames;
use crate::members::stale_peers::{self, Evidence};
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry::{Telemetry, TelemetryView};
use crate::proxy::domains::DomainList;
use crate::proxy::route::RouteCtx;
use crate::site_checks::checks::Checks;
use crate::transport::peers_cache::PeersCache;

/// The first sweep is 10 minutes after start.
pub const FIRST_PRUNE_AFTER: Duration = Duration::from_secs(600);
/// Then once an hour.
pub const PRUNE_EVERY: Duration = Duration::from_secs(3600);
/// The timeout for each document read inside a tick.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// What was learned from the shared document.
pub struct DocEvidence {
    /// The time of the freshest check of each author.
    pub check_seen_unix: HashMap<PeerId, u64>,
    /// The time of each author's last write.
    pub doc_seen_unix: HashMap<PeerId, u64>,
}

/// The live state of the node at the moment of a tick.
pub struct LiveEvidence {
    pub id: PeerId,
    pub members: HashSet<PeerId>,
    pub tel: TelemetryView,
    pub now: Instant,
    pub chosen_exits: Vec<PeerId>,
}

/// The sources for `prune_tick`: everything a tick reads.
pub struct PruneSources<'a> {
    pub id: PeerId,
    pub members: &'a Members,
    pub telemetry: &'a Telemetry,
    pub domains: &'a DomainList,
    pub checks: &'a Checks,
    pub route: &'a RouteCtx,
}

/// Sweep candidates: members of the cache and of the names, except itself, network members and chosen exits,
/// that are "gone" by `stale_peers::is_gone`; the value is the age.
fn candidates(
    doc: &DocEvidence,
    live: &LiveEvidence,
    cache: &PeersCache,
    names: &PeerNames,
    now_unix: u64,
) -> HashMap<PeerId, Option<u64>> {
    let seen = cache.seen();
    let ev = Evidence {
        members: &live.members,
        telemetry: &live.tel,
        now: live.now,
        now_unix,
        check_seen_unix: &doc.check_seen_unix,
        cache_seen_unix: &seen,
        doc_seen_unix: &doc.doc_seen_unix,
    };
    let known: HashSet<PeerId> = seen.keys().copied().chain(names.ids()).collect();
    known
        .into_iter()
        .filter(|x| *x != live.id && !live.chosen_exits.contains(x))
        .map(|x| (x, stale_peers::age_sec(&ev, x)))
        .filter(|(_, age)| stale_peers::is_gone(*age))
        .collect()
}

/// Removes `gone` from the cache and the names and writes to disk only what changed.
fn forget_and_save(gone: &HashSet<PeerId>, cache: &mut PeersCache, names: &mut PeerNames) {
    if cache.forget(gone) {
        if let Err(e) = cache.save() {
            say("cache", format!("address cache not saved: {e}"));
        }
    }
    if names.forget(gone) {
        if let Err(e) = names.save() {
            say("cache", format!("member names not saved: {e}"));
        }
    }
}

/// Cuts the hidden nodes (removed from the member list here) from the cache and the names at once.
/// Returns those that were present in either of them.
pub fn cut_hidden(
    hidden: &BTreeSet<PeerId>,
    cache: &mut PeersCache,
    names: &mut PeerNames,
) -> HashSet<PeerId> {
    let known: HashSet<PeerId> = cache.seen().keys().copied().chain(names.ids()).collect();
    let cut: HashSet<PeerId> = hidden
        .iter()
        .copied()
        .filter(|p| known.contains(p))
        .collect();
    forget_and_save(&cut, cache, names);
    cut
}

/// One sweep over what has already been read: cuts the intersection of the candidates with the previous `prune_candidates_prev_tick`, then
/// `prune_candidates_prev_tick` = the current candidates. A read error is `Err`, nothing is cut and `prune_candidates_prev_tick` stays as it was.
/// Returns the cut ones with their age.
pub fn prune_from(
    read: Result<DocEvidence, String>,
    live: &LiveEvidence,
    prune_candidates_prev_tick: &mut HashSet<PeerId>,
    cache: &mut PeersCache,
    names: &mut PeerNames,
    now_unix: u64,
) -> Result<Vec<(PeerId, Option<u64>)>, String> {
    let doc = read?;
    let found = candidates(&doc, live, cache, names, now_unix);
    let pruned: Vec<(PeerId, Option<u64>)> = found
        .iter()
        .filter(|(x, _)| prune_candidates_prev_tick.contains(*x))
        .map(|(x, age)| (*x, *age))
        .collect();
    *prune_candidates_prev_tick = found.into_keys().collect();
    let gone: HashSet<PeerId> = pruned.iter().map(|c| c.0).collect();
    forget_and_save(&gone, cache, names);
    Ok(pruned)
}

/// A read under a timeout: any failure is the text of the reason.
async fn bounded<T>(what: &str, fut: impl Future<Output = anyhow::Result<T>>) -> Result<T, String> {
    match tokio::time::timeout(READ_TIMEOUT, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(format!("{what}: {e}")),
        Err(_) => Err(format!("{what}: timeout")),
    }
}

/// Reads the document: the domain list, the checks, the entry stamps.
async fn read_docs(src: &PruneSources<'_>, now_unix: u64) -> Result<DocEvidence, String> {
    let list = bounded("domain list", src.domains.list()).await?;
    let views = bounded("checks", src.checks.check_views()).await?;
    let r = src.domains.shared_replica();
    let doc_seen_unix = bounded(
        "entry stamps",
        stale_peers::read_doc_seen(r.as_ref(), now_unix),
    )
    .await?;
    Ok(DocEvidence {
        check_seen_unix: stale_peers::latest_check_unix_by_author(&views, &list),
        doc_seen_unix,
    })
}

/// A log line about a cut member.
fn say_pruned(id: &PeerId, age: Option<u64>) {
    match age {
        Some(a) => say(
            "cache",
            format!(
                "{} removed from cache: not seen for {} d",
                short_peer_id(id),
                a / 86_400
            ),
        ),
        None => say(
            "cache",
            format!("{} removed from cache: no signs of life", short_peer_id(id)),
        ),
    }
}

/// A sweep tick: the reads (any failure is a log line, nothing is cut), `prune_from`,
/// a log line for each cut member. Returns the ids of the cut ones (empty on a read failure):
/// the caller forgets them in the redial.
pub async fn prune_tick(
    src: &PruneSources<'_>,
    prune_candidates_prev_tick: &mut HashSet<PeerId>,
    cache: &mut PeersCache,
    names: &mut PeerNames,
    now_unix: u64,
) -> HashSet<PeerId> {
    let hidden = src
        .members
        .access()
        .verdicts()
        .map(|v| v.removed_here())
        .unwrap_or_default();
    let mut cut = cut_hidden(&hidden, cache, names);
    cut.extend(sweep(src, prune_candidates_prev_tick, cache, names, now_unix).await);
    cut
}

/// The age-based part of a tick: the reads, `prune_from`, a log line for each cut member.
async fn sweep(
    src: &PruneSources<'_>,
    prune_candidates_prev_tick: &mut HashSet<PeerId>,
    cache: &mut PeersCache,
    names: &mut PeerNames,
    now_unix: u64,
) -> HashSet<PeerId> {
    let read = read_docs(src, now_unix).await;
    let live = LiveEvidence {
        id: src.id,
        members: src.members.ids(),
        tel: src.telemetry.view(),
        now: Instant::now(),
        chosen_exits: src.route.chosen_exits(),
    };
    match prune_from(
        read,
        &live,
        prune_candidates_prev_tick,
        cache,
        names,
        now_unix,
    ) {
        Ok(pruned) => {
            pruned.iter().for_each(|(id, age)| say_pruned(id, *age));
            pruned.into_iter().map(|(id, _)| id).collect()
        }
        Err(e) => {
            say("cache", format!("sweep skipped: {e}"));
            HashSet::new()
        }
    }
}

#[cfg(test)]
#[path = "stale_prune_tests.rs"]
mod tests;
