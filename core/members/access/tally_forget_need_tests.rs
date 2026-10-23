//! Tests of the threshold the forget pass reports (`Ledger::forget_need`), which counts the nodes that only a
//! dead admission lets in and so may differ from the exclusion threshold `Ledger::need`.

use crate::members::access::record::Via;
use crate::members::access::tally::{tally, AdmitIn, ForgetIn, TallyInput};
use crate::transport::net::contract::pid;

#[test]
fn forget_need_counts_nodes_admitted_by_a_dead_admission() {
    // given: me admitted a and b; me and b forgot a at 100; then a admitted e at 150 (a dead admission)
    let (me, a, b, e) = (pid(1), pid(2), pid(3), pid(4));
    let admit = |author, subject, made_unix| AdmitIn {
        author,
        subject,
        via: Via::Invite,
        made_unix,
    };
    let vote = |author, subject, made_unix| ForgetIn {
        author,
        subject,
        made_unix,
    };
    let ledger = tally(&TallyInput {
        me,
        admits: vec![admit(me, a, 0), admit(me, b, 0), admit(a, e, 150)],
        votes: Vec::new(),
        forgets: vec![vote(me, a, 100), vote(b, a, 100)],
    });
    // when / then: a is forgotten and e is no member, yet e is among those the forget pass counts
    assert!(ledger.is_forgotten(&a));
    assert!(!ledger.is_member(&e));
    assert_eq!(
        ledger.need, 1,
        "the exclusion threshold counts live admissions only"
    );
    assert_eq!(
        ledger.forget_need, 2,
        "the forget threshold counts e as well"
    );
}
