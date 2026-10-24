//! Tests of `Verdicts::removed_here`: the set of subjects hidden from this node never holds the node itself.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::members::access::tally::{tally, ForgetRound, Rules, TallyInput};
use crate::members::access::view::Verdicts;
use crate::transport::net::contract::pid;

#[test]
fn removed_here_never_lists_me() {
    // given: I am forgotten, and so is another node
    let (me, other) = (pid(1), pid(2));
    let mut ledger = tally(&TallyInput {
        me,
        admits: Vec::new(),
        votes: Vec::new(),
        forgets: Vec::new(),
        rules: Rules::Peer,
    });
    for subject in [me, other] {
        let round = ForgetRound {
            voters: vec![pid(3)],
            at_unix: Some(100),
        };
        ledger.forgets.insert(subject, round);
    }
    let v = Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    };
    // when / then: the other one is hidden, I am not
    assert_eq!(v.removed_here(), BTreeSet::from([other]));
}
