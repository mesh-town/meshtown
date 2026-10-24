//! The pure part of the boot: which admissions to write and which witnesses become votes. No replica, no clock.

use std::collections::BTreeSet;

use crate::transport::net::{PeerId, ReplicaEntry};

use super::authority::Rules;
use super::boot::{AccessBoot, MAX_BOOT_ADMITS};
use super::record::{tally_input_under, AclKind, Via};
use super::subjects::{live_subjects, withdrawn_subjects, witnessed_subjects};
use super::tally::{tally, AdmitIn, Ledger};

/// What a boot is going to write.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct BootPlan {
    pub witness: Vec<PeerId>,
    pub skipped: usize,
    pub ignored: usize,
    pub me_left: bool,
    /// Own witnesses that become votes at the first start of this version.
    pub promote: Vec<PeerId>,
}

fn unique(ids: impl Iterator<Item = PeerId>) -> Vec<PeerId> {
    let mut seen = BTreeSet::new();
    ids.filter(|p| seen.insert(*p)).collect()
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
    out.extend(live_subjects(snap, me, AclKind::Forget));
    out.extend(ledger.forgets.keys().filter(|p| ledger.is_forgotten(p)));
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
        *p != me
            && !refused.contains(p)
            && !blocks.contains(p)
            && !ledger.excluded.contains_key(p)
            && !ledger.forgets.contains_key(p)
    });
    out.into_iter().collect()
}

fn keep(ids: Vec<PeerId>, refused: &BTreeSet<PeerId>, me_left: bool) -> Vec<PeerId> {
    if me_left {
        return Vec::new();
    }
    ids.into_iter().filter(|p| !refused.contains(p)).collect()
}

/// The ceiling on witnesses; returns what is left over.
fn cap(mut witness: Vec<PeerId>) -> BootPlan {
    let skipped = witness.len().saturating_sub(MAX_BOOT_ADMITS);
    witness.truncate(MAX_BOOT_ADMITS);
    BootPlan {
        witness,
        skipped,
        ..BootPlan::default()
    }
}

/// Filter, tally, ceiling, order.
pub(super) fn plan_admits(
    snap: &[ReplicaEntry],
    blocks: &BTreeSet<PeerId>,
    b: &AccessBoot,
) -> BootPlan {
    let witness = unique(b.initial.iter().chain(&b.cached).copied());
    let (mut input, ignored) = tally_input_under(Rules::Peer, b.me, snap);
    input.admits.extend(witness.iter().map(|s| AdmitIn {
        author: b.me,
        subject: *s,
        via: Via::Witness,
        made_unix: b.now_unix,
    }));
    let ledger = tally(&input);
    let me_left = ledger.excluded.contains_key(&b.me);
    let refused = refused_set(snap, b.me, blocks, &ledger);
    let plan = cap(keep(witness, &refused, me_left));
    let promote = promotions(snap, b.me, blocks, &ledger, &plan.witness);
    BootPlan {
        ignored,
        me_left,
        promote,
        ..plan
    }
}

#[cfg(test)]
#[path = "boot_forget_tests.rs"]
mod forget_tests;
