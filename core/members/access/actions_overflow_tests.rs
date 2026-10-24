//! Test of the removal vote stamp (`actions.rs`) when a replicated admission carries the largest time.

use std::sync::Arc;

use super::*;
use crate::members::access::tally::{tally, AdmitIn, Rules, TallyInput};
use crate::transport::net::contract::pid;

#[test]
fn forget_vote_after_an_admission_at_the_largest_time_does_not_overflow() {
    // given: the subject was admitted at the largest possible time
    let (me, subject) = (pid(1), pid(2));
    let admit = AdmitIn {
        author: me,
        subject,
        via: Via::Invite,
        made_unix: u64::MAX,
    };
    let ledger = tally(&TallyInput {
        me,
        admits: vec![admit],
        votes: Vec::new(),
        forgets: Vec::new(),
        rules: Rules::Peer,
    });
    let verdicts = Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: Default::default(),
        blocks: Default::default(),
        reasons: Default::default(),
    };
    // when / then: the stamp saturates
    assert_eq!(forget_vote(&verdicts, subject).made_unix, u64::MAX);
}
