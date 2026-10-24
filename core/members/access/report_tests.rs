//! Tests of the member list reply (`report.rs`): row order, the fields of votes, exclusions and refusals,
//! the `denied_by_me` codes and the counters. The verdicts are built by hand, there is no replica here.
//! Nodes: `me` is the own node (seed 1); the others are named by what they are in the test.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::members::access::tally::{Exclusion, Ledger, PendingVote};
use crate::members::access::view::Deny;
use crate::transport::net::contract::pid;
use crate::web_gui::MemberRow;

const AT_UNIX: u64 = 1_792_400_000;

#[path = "report_rows_tests.rs"]
mod rows;

#[path = "report_totals_tests.rs"]
mod totals_and_codes;

// --- setup and helpers ---

struct World {
    ledger: Ledger,
    verdicts: Verdicts,
    members: Members,
    refusals: Vec<(PeerId, Refusal)>,
}

/// The node (seed 1) with `listed` and `excluded` admitted by the list (the excluded are then excluded by votes).
fn world(listed: &[PeerId], excluded: &[PeerId]) -> World {
    let me = pid(1);
    let admitted: BTreeSet<PeerId> = [&[me][..], listed, excluded].concat().into_iter().collect();
    let ledger = Ledger {
        admitted,
        excluded: BTreeMap::new(),
        pending: BTreeMap::new(),
        need: 2,
        admissions: BTreeMap::new(),
        forgets: Default::default(),
        admit_unix: Default::default(),
        forget_marks: Default::default(),
        forget_need: 1,
        admins: Default::default(),
        closed: None,
    };
    let verdicts = Verdicts {
        me,
        ledger: Arc::new(ledger.clone()),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    };
    let mut w = World {
        ledger,
        verdicts,
        members: Members::default(),
        refusals: Vec::new(),
    };
    excluded
        .iter()
        .for_each(|p| w.exclude(*p, How::Votes, vec![pid(2)]));
    w
}

impl World {
    fn exclude(&mut self, peer: PeerId, how: How, voters: Vec<PeerId>) {
        self.ledger.excluded.insert(
            peer,
            Exclusion {
                how,
                at_micros: AT_UNIX * 1_000_000 + 5,
                voters,
            },
        );
    }

    fn pending(&mut self, peer: PeerId, voters: Vec<PeerId>, need: u8) {
        self.ledger
            .pending
            .insert(peer, PendingVote { voters, need });
    }

    fn refusal(&mut self, peer: PeerId, side: Side, deny: Deny) {
        self.refusals.push((
            peer,
            Refusal {
                side,
                deny,
                at_unix: AT_UNIX + 100,
            },
        ));
    }

    fn verdicts(&self) -> Verdicts {
        Verdicts {
            ledger: Arc::new(self.ledger.clone()),
            ..self.verdicts.clone()
        }
    }

    fn reply(&self) -> crate::web_gui::MembersReply {
        members_reply(&self.verdicts(), &self.members, &self.refusals, None)
    }

    fn row(&self, peer: PeerId) -> MemberRow {
        let rows = self.reply().members;
        rows.into_iter()
            .find(|r| r.id_full == peer.to_string())
            .expect("the row must exist")
    }
}
