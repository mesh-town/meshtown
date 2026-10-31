//! Tests of the member list reply (`report.rs`): row order, the fields of exclusions and refusals,
//! the `denied_by_me` codes and the counters. The verdicts are built by hand, there is no replica here.
//! Nodes: `me` is the own node (seed 1); the others are named by what they are in the test.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::members::access::tally::{Exclusion, Ledger};
use crate::members::access::view::Deny;
use crate::transport::net::contract::pid;
use crate::web_gui::MemberRow;

const AT_UNIX: u64 = 1_792_400_000;

#[path = "report_rows_tests.rs"]
mod rows;

#[path = "report_totals_tests.rs"]
mod totals_and_codes;

mod me_excluded {
    use super::*;

    #[test]
    fn me_excluded_by_the_list() {
        // given: the list excluded this node
        let w = world(&[pid(2)], &[pid(1)]);
        // then
        assert!(w.reply().me_excluded);
    }

    #[test]
    fn me_excluded_by_refusals_when_nobody_is_online() {
        // given: nobody is connected and a neighbour turned this node away as excluded
        let mut w = world(&[pid(2)], &[]);
        w.refusal(pid(2), Side::Them, Deny::Excluded);
        // then
        assert!(w.reply().me_excluded);
    }

    #[test]
    fn an_online_member_outweighs_refusals() {
        // given: one neighbour refuses as excluded, another one is connected
        let mut w = world(&[pid(2), pid(3)], &[]);
        w.refusal(pid(2), Side::Them, Deny::Excluded);
        w.members.add_verified_conn(pid(3));
        // then
        assert!(
            !w.reply().me_excluded,
            "a connected member contradicts the refusals"
        );
    }

    #[test]
    fn other_refusals_do_not_mark_me_excluded() {
        let cases = [
            (Side::Them, Some(Deny::Left)),
            (Side::Them, Some(Deny::Blocked)),
            (Side::Them, Some(Deny::Refused)),
            (Side::Them, Some(Deny::Unlisted)),
            (Side::Me, Some(Deny::Excluded)),
            (Side::Me, None),
        ];
        for (side, deny) in cases {
            // given: nobody is online and the only refusal is the case (or there is none)
            let mut w = world(&[pid(2)], &[]);
            if let Some(deny) = deny {
                w.refusal(pid(2), side, deny);
            }
            // then
            assert!(!w.reply().me_excluded, "{side:?} {deny:?}");
        }
    }
}

// --- setup and helpers ---

struct World {
    ledger: Ledger,
    verdicts: Verdicts,
    members: Members,
    refusals: Vec<(PeerId, Refusal)>,
}

/// The node (seed 1) with `listed` and `excluded` admitted by the list .
fn world(listed: &[PeerId], excluded: &[PeerId]) -> World {
    let me = pid(1);
    let admitted: BTreeSet<PeerId> = [&[me][..], listed, excluded].concat().into_iter().collect();
    let ledger = Ledger {
        admitted,
        excluded: BTreeMap::new(),
        admissions: BTreeMap::new(),
        admins: Default::default(),
        closed: None,
    };
    let verdicts = Verdicts {
        me,
        ledger: Arc::new(ledger.clone()),
        blocks: BTreeSet::new(),
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
        members_reply(&self.verdicts(), &self.members, "", &self.refusals, None)
    }

    fn row(&self, peer: PeerId) -> MemberRow {
        let rows = self.reply().members;
        rows.into_iter()
            .find(|r| r.id_full == peer.to_string())
            .expect("the row must exist")
    }
}
