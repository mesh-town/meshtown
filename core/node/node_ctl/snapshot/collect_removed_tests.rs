//! `removed_of_verdicts`: the nodes hidden from this node's member list.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::members::access::tally::{tally, ForgetRound, Rules, TallyInput};
use crate::members::access::view::Verdicts;
use crate::transport::net::contract::pid;

#[test]
fn removed_of_without_verdicts_empty() {
    assert!(removed_of_verdicts(None).is_empty());
}

#[test]
fn removed_of_forgotten_and_my_pending_votes() {
    // given: one completed forget, one pending forget I voted in, one of me
    let (me, done, mine, other) = (pid(1), pid(2), pid(3), pid(4));
    let mut ledger = tally(&TallyInput {
        me,
        admits: Vec::new(),
        votes: Vec::new(),
        forgets: Vec::new(),
        rules: Rules::Peer,
        powers: vec![],
    });
    let round = |voters: Vec<PeerId>, at_unix| ForgetRound { voters, at_unix };
    ledger.forgets.insert(done, round(vec![pid(9)], Some(100)));
    ledger.forgets.insert(mine, round(vec![me], None));
    ledger.forgets.insert(other, round(vec![pid(9)], None));
    ledger.forgets.insert(me, round(vec![pid(9)], Some(100)));
    let v = Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: BTreeMap::new(),
    };
    // when / then
    let got = removed_of_verdicts(Some(Arc::new(v)));
    assert_eq!(got, HashSet::from([done, mine]));
}
