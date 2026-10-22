//! Boot of the membership list: confirms the own "admitted" records for the known neighbours at every
//! start and, once per node, migrates the previous participants into the list.
//!
//! What it does: one run reads the shared replica first, then writes the witnesses (`initial` and `cached`
//! neighbours), on the first start of a long-time node the recent authors, and the boot marker last.
//! What it doesn't do: no watching and no verdicts (`runtime`, `watch`); the caller logs a failure.

use std::collections::BTreeSet;

use crate::node::output::say;
use crate::transport::net::{NetResult, PeerId, Replica, ReplicaEntry};

use super::boot_plan::{is_legacy, plan_admits, BootPlan};
use super::record::{
    self, parse_entry, AclKind, AclValue, Parsed, Via, BLOCK_PREFIX, BOOT_KEY, BOOT_NOTE_ADMISSION,
};
use super::subjects::live_subjects;

pub(super) const MIGRATION_LOOKBACK_SECS: u64 = 30 * 24 * 3600;
pub(super) const MAX_BOOT_ADMITS: usize = 256;

/// Input of one boot.
pub struct AccessBoot {
    pub me: PeerId,
    pub initial: Vec<PeerId>,
    pub cached: Vec<PeerId>,
    pub now_unix: u64,
}

/// The kind of run, for the log line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Legacy,
    Fresh,
    Done,
    Left,
}

fn class_of(first: bool, legacy: bool, plan: &BootPlan) -> Class {
    match (plan.me_left, first, legacy) {
        (true, _, _) => Class::Left,
        (_, false, _) => Class::Done,
        (_, true, true) => Class::Legacy,
        _ => Class::Fresh,
    }
}

/// What the run does with the marker of the node.
enum Mark {
    /// First start of a new node: the marker with the flag goes first, nothing is promoted.
    Newcomer,
    /// First start of a node of the first stage: promote, then the marker with the flag.
    Legacy,
    /// A readable marker without the flag: promote, then the same marker with the flag.
    Veteran(AclValue),
    /// The flag is set, or the marker cannot be read: nothing to do.
    Settled,
}

fn mark_of(first: bool, legacy: bool, entry: Option<&ReplicaEntry>) -> Mark {
    match (first, legacy) {
        (true, true) => Mark::Legacy,
        (true, false) => Mark::Newcomer,
        _ => entry.map_or(Mark::Settled, veteran_of),
    }
}

fn veteran_of(entry: &ReplicaEntry) -> Mark {
    match parse_entry(entry) {
        Parsed::Record { value, .. }
            if value.kind == AclKind::Boot && value.note != BOOT_NOTE_ADMISSION =>
        {
            Mark::Veteran(value)
        }
        _ => Mark::Settled,
    }
}

fn value(kind: AclKind, subject: PeerId, via: Via, now_unix: u64) -> AclValue {
    AclValue {
        kind,
        subject,
        via,
        made_unix: now_unix,
        note: String::new(),
    }
}

/// Writes the admissions one after another; the first error stops the rest.
async fn write_admits(
    shared: &dyn Replica,
    ids: &[PeerId],
    via: Via,
    now_unix: u64,
) -> NetResult<usize> {
    let mut written = 0;
    for id in ids {
        written +=
            usize::from(record::ensure(shared, &value(AclKind::Admit, *id, via, now_unix)).await?);
    }
    Ok(written)
}

fn tail(plan: &BootPlan) -> String {
    let skipped = (plan.skipped > 0).then(|| format!(", skipped {}", plan.skipped));
    let ignored = (plan.ignored > 0).then(|| format!(", ignored {}", plan.ignored));
    skipped.into_iter().chain(ignored).collect()
}

/// The one log line of a run: English, numbers only.
fn boot_line(class: Class, plan: &BootPlan, written: usize) -> String {
    let head = match class {
        Class::Legacy => {
            let (w, m) = (plan.witness.len(), plan.migration.len());
            format!("acl boot: legacy, admitted {written} (witness {w}, migration {m})")
        }
        Class::Fresh => format!("acl boot: fresh, admitted {written}"),
        Class::Done => format!("acl boot: done earlier, admitted {written}"),
        Class::Left => "acl boot: left the network, nothing admitted".to_string(),
    };
    format!("{head}{}", tail(plan))
}

/// Everything read before the first write of the run.
struct Reads {
    snap: Vec<ReplicaEntry>,
    marker_entry: Option<ReplicaEntry>,
    blocks: BTreeSet<PeerId>,
}

async fn read_all(shared: &dyn Replica, own: &dyn Replica, me: PeerId) -> NetResult<Reads> {
    let snap = shared.scan(b"").await?;
    let marker_entry = shared.get(me, BOOT_KEY).await?;
    let blocks = live_subjects(&own.scan(BLOCK_PREFIX).await?, me, AclKind::Block);
    Ok(Reads {
        snap,
        marker_entry,
        blocks,
    })
}

/// Runs the boot: the marker of a newcomer first, then the witnesses, then the migration (first start of a
/// long-time node only), then the one-time promotion of the own witnesses to votes, and the marker of a
/// long-time node last. Returns how many admissions were written. A newcomer marker that turns out to be there
/// already (synced in after the read) restarts the run once from a fresh read. A failed write of a
/// long-time node leaves no marker, so the next start repeats the whole run (the records are idempotent);
/// a newcomer is marked before anything else, so a failed run cannot make it look like a long-time node.
pub async fn run_boot(shared: &dyn Replica, own: &dyn Replica, b: &AccessBoot) -> NetResult<usize> {
    match run_once(shared, own, b).await? {
        Some(written) => Ok(written),
        None => Ok(run_once(shared, own, b).await?.unwrap_or(0)),
    }
}

/// One pass of the boot. `None` — the marker of a newcomer was already there (it arrived by sync after the
/// read), nothing else was written and no line was logged: the caller reads again and decides anew.
async fn run_once(
    shared: &dyn Replica,
    own: &dyn Replica,
    b: &AccessBoot,
) -> NetResult<Option<usize>> {
    let reads = read_all(shared, own, b.me).await?;
    let first = reads.marker_entry.is_none();
    let legacy = first && is_legacy(&reads.snap, b.me);
    let plan = plan_admits(&reads.snap, &reads.blocks, b, legacy.then_some(b.now_unix));
    let mark = mark_of(first, legacy, reads.marker_entry.as_ref());
    let marked = write_marker(shared, matches!(mark, Mark::Newcomer), Via::None, b).await?;
    if matches!(mark, Mark::Newcomer) && !marked {
        return Ok(None);
    }
    let witnessed = write_admits(shared, &plan.witness, Via::Witness, b.now_unix).await?;
    let migrated = write_admits(shared, &plan.migration, Via::Migration, b.now_unix).await?;
    let promoted = promote(shared, &mark, &plan, b).await?;
    finish_marker(shared, &mark, b).await?;
    let written = witnessed + migrated;
    let line = boot_line(class_of(first, legacy, &plan), &plan, written);
    say(
        "access",
        if promoted > 0 {
            format!("{line}, promoted {promoted}")
        } else {
            line
        },
    );
    Ok(Some(written))
}

/// Turns the own witnesses of the plan into votes, when the mark says a promotion is due.
async fn promote(
    shared: &dyn Replica,
    mark: &Mark,
    plan: &BootPlan,
    b: &AccessBoot,
) -> NetResult<usize> {
    if !matches!(mark, Mark::Legacy | Mark::Veteran(_)) {
        return Ok(0);
    }
    write_admits(shared, &plan.promote, Via::None, b.now_unix).await
}

/// Writes the marker when due; `false` — not due, or a live marker of the node was already there.
async fn write_marker(
    shared: &dyn Replica,
    due: bool,
    via: Via,
    b: &AccessBoot,
) -> NetResult<bool> {
    if !due {
        return Ok(false);
    }
    let mut marker = value(AclKind::Boot, b.me, via, b.now_unix);
    marker.note = BOOT_NOTE_ADMISSION.to_string();
    record::ensure(shared, &marker).await
}

/// The flag goes last: a failed promotion leaves the marker as it was and the next start repeats it.
async fn finish_marker(shared: &dyn Replica, mark: &Mark, b: &AccessBoot) -> NetResult<()> {
    match mark {
        Mark::Legacy => write_marker(shared, true, Via::Migration, b)
            .await
            .map(|_| ()),
        Mark::Veteran(old) => {
            record::rewrite(
                shared,
                &AclValue {
                    note: BOOT_NOTE_ADMISSION.to_string(),
                    ..old.clone()
                },
            )
            .await
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;
