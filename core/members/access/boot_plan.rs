//! The pure part of the boot migration: who is a long-time node, which authors are recent, which
//! admissions to write. No replica, no clock.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::transport::net::{PeerId, ReplicaEntry};

use super::boot::{AccessBoot, MAX_BOOT_ADMITS, MIGRATION_LOOKBACK_SECS};
use super::record::{tally_input, AclKind, Via, ACL_PREFIX};
use super::subjects::{live_subjects, withdrawn_subjects, witnessed_subjects};
use super::tally::{tally, AdmitIn, Ledger};

/// What a boot is going to write.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct BootPlan {
    pub witness: Vec<PeerId>,
    pub migration: Vec<PeerId>,
    pub skipped: usize,
    pub ignored: usize,
    pub me_left: bool,
    /// Own witnesses that become votes at the first start of this version.
    pub promote: Vec<PeerId>,
}

/// The node wrote something to the shared replica before (a record outside `acl/`, a tombstone too).
pub(super) fn is_legacy(snap: &[ReplicaEntry], me: PeerId) -> bool {
    snap.iter()
        .any(|e| e.author == me && !e.key.starts_with(ACL_PREFIX))
}

/// Authors (not `me`) with their latest record time inside the 30 days before `made_unix`.
pub(super) fn recent_authors(
    snap: &[ReplicaEntry],
    me: PeerId,
    made_unix: u64,
) -> Vec<(PeerId, u64)> {
    let hi = made_unix.saturating_mul(1_000_000);
    let lo = hi.saturating_sub(MIGRATION_LOOKBACK_SECS * 1_000_000);
    let mut latest: BTreeMap<PeerId, u64> = BTreeMap::new();
    for e in snap
        .iter()
        .filter(|e| e.author != me && e.ts_micros >= lo && e.ts_micros < hi)
    {
        let t = latest.entry(e.author).or_insert(0);
        *t = (*t).max(e.ts_micros);
    }
    latest.into_iter().collect()
}

fn unique(ids: impl Iterator<Item = PeerId>) -> Vec<PeerId> {
    let mut seen = BTreeSet::new();
    ids.filter(|p| seen.insert(*p)).collect()
}

/// Recent authors, newest first (equal times by id bytes), without those already in `witness`.
fn migration_candidates(
    snap: &[ReplicaEntry],
    me: PeerId,
    made: Option<u64>,
    witness: &[PeerId],
) -> Vec<PeerId> {
    let Some(made_unix) = made else {
        return Vec::new();
    };
    let mut authors = recent_authors(snap, me, made_unix);
    authors.sort_by_key(|(p, ts)| (Reverse(*ts), *p));
    authors
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| !witness.contains(p))
        .collect()
}

/// Nodes that must not get a new admission: admitted or withdrawn by me, excluded, voted by me, blocked.
fn refused_set(
    snap: &[ReplicaEntry],
    me: PeerId,
    blocks: &BTreeSet<PeerId>,
    ledger: &Ledger,
) -> BTreeSet<PeerId> {
    let mut out = live_subjects(snap, me, AclKind::Admit);
    out.extend(live_subjects(snap, me, AclKind::Exclude));
    out.extend(withdrawn_subjects(snap, me, AclKind::Admit));
    out.extend(ledger.excluded.keys().copied());
    out.extend(blocks.iter().copied());
    out.insert(me);
    out
}

/// Own live witnesses and the witnesses of this run, minus excluded, blocked, voted against and me.
pub(super) fn promotions(
    snap: &[ReplicaEntry],
    me: PeerId,
    blocks: &BTreeSet<PeerId>,
    ledger: &Ledger,
    witness: &[PeerId],
) -> Vec<PeerId> {
    if ledger.excluded.contains_key(&me) {
        return Vec::new();
    }
    let mut out = witnessed_subjects(snap, me);
    out.extend(witness.iter().copied());
    let refused = live_subjects(snap, me, AclKind::Exclude);
    out.retain(|p| {
        *p != me && !refused.contains(p) && !blocks.contains(p) && !ledger.excluded.contains_key(p)
    });
    out.into_iter().collect()
}

fn keep(ids: Vec<PeerId>, refused: &BTreeSet<PeerId>, me_left: bool) -> Vec<PeerId> {
    if me_left {
        return Vec::new();
    }
    ids.into_iter().filter(|p| !refused.contains(p)).collect()
}

/// Witnesses first, then the migration list; returns what is left over past the ceiling.
fn cap(mut witness: Vec<PeerId>, mut migration: Vec<PeerId>) -> BootPlan {
    let over_witness = witness.len().saturating_sub(MAX_BOOT_ADMITS);
    witness.truncate(MAX_BOOT_ADMITS);
    let room = MAX_BOOT_ADMITS - witness.len();
    let over_migration = migration.len().saturating_sub(room);
    migration.truncate(room);
    BootPlan {
        witness,
        migration,
        skipped: over_witness + over_migration,
        ..BootPlan::default()
    }
}

/// Filter, tally, ceiling, order. `migrate` is the time the 30-day lookback ends at, when the migration is on.
pub(super) fn plan_admits(
    snap: &[ReplicaEntry],
    blocks: &BTreeSet<PeerId>,
    b: &AccessBoot,
    migrate: Option<u64>,
) -> BootPlan {
    let witness = unique(b.initial.iter().chain(&b.cached).copied());
    let migration = migration_candidates(snap, b.me, migrate, &witness);
    let (mut input, ignored) = tally_input(b.me, snap);
    let intended = witness.iter().chain(&migration);
    input.admits.extend(intended.map(|s| AdmitIn {
        author: b.me,
        subject: *s,
        via: Via::Witness,
    }));
    let ledger = tally(&input);
    let me_left = ledger.excluded.contains_key(&b.me);
    let refused = refused_set(snap, b.me, blocks, &ledger);
    let plan = cap(
        keep(witness, &refused, me_left),
        keep(migration, &refused, me_left),
    );
    let promote = promotions(snap, b.me, blocks, &ledger, &plan.witness);
    BootPlan {
        ignored,
        me_left,
        promote,
        ..plan
    }
}
