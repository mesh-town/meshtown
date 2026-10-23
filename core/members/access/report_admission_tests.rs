//! Tests of the requests in the member list reply: the `pending` status, the `admission` object, the node's own
//! request, the counters and the summary. The verdicts are built by hand; `me` is the node with seed 1.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::members::access::tally::{Admission, How, Ledger};
use crate::transport::net::contract::pid;

const AT_UNIX: u64 = 1_792_400_000;

fn request(voters: &[PeerId], label: Option<&str>) -> Admission {
    Admission {
        voters: voters.to_vec(),
        need: 2,
        label: label.map(str::to_string),
    }
}

/// Me and `members` are admitted; `asks` are requests that wait.
fn verdicts(members: &[PeerId], asks: Vec<(PeerId, Admission)>) -> Verdicts {
    let admitted: BTreeSet<PeerId> = [&[pid(1)][..], members].concat().into_iter().collect();
    let ledger = Ledger {
        admitted,
        excluded: BTreeMap::new(),
        pending: BTreeMap::new(),
        need: 2,
        admissions: asks.into_iter().collect(),
        forgets: Default::default(),
        admit_unix: Default::default(),
        forget_marks: Default::default(),
    };
    Verdicts {
        me: pid(1),
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    }
}

fn reply(v: &Verdicts, refusals: &[(PeerId, Refusal)]) -> crate::web_gui::MembersReply {
    members_reply(v, &Members::default(), refusals, None)
}

fn row_of(r: &crate::web_gui::MembersReply, p: PeerId) -> MemberRow {
    r.members
        .iter()
        .find(|row| row.id_full == p.to_string())
        .cloned()
        .expect("the row must exist")
}

fn unlisted_by_them() -> Vec<(PeerId, Refusal)> {
    vec![(
        pid(2),
        Refusal {
            side: Side::Them,
            deny: Deny::Unlisted,
            at_unix: AT_UNIX,
        },
    )]
}

#[test]
fn pending_row_carries_admission_and_label() {
    // given: a request of x with one confirmation of a, a note on the invitation
    let (a, x) = (pid(2), pid(9));
    let v = verdicts(&[a], vec![(x, request(&[a], Some("Anna's laptop")))]);
    // when: the reply is built
    let row = row_of(&reply(&v, &[]), x);
    // then: the row is pending and says who confirmed
    assert_eq!(row.status, "pending");
    let info = row.admission.expect("a request has the admission object");
    assert_eq!((info.votes, info.need, info.mine), (1, 2, false));
    assert_eq!(info.voters, vec![a.to_string()]);
    assert_eq!(info.label.as_deref(), Some("Anna's laptop"));
}

#[test]
fn member_held_by_my_vote_shows_admission() {
    // given: a is admitted but needs one more confirmation besides mine
    let a = pid(2);
    let v = verdicts(&[a], vec![(a, request(&[pid(1)], None))]);
    // when / then: the row stays a member and carries the request with my vote
    let row = row_of(&reply(&v, &[]), a);
    assert_eq!(row.status, "member");
    let info = row
        .admission
        .expect("a short member carries the admission object");
    assert!(info.mine);
}

#[test]
fn me_pending_needs_a_refusal_from_them() {
    // given: my own request is on the list
    let v = verdicts(&[], vec![(pid(1), request(&[], None))]);
    // when: nobody has turned me away yet
    let quiet = reply(&v, &[]);
    // then: I am not shown as waiting
    assert!(!quiet.me_pending);
    assert!(quiet.me_admission.is_none());
    // when: another node refused me as not admitted
    let refused = reply(&v, &unlisted_by_them());
    // then: I wait, with my request shown
    assert!(refused.me_pending);
    assert_eq!(
        refused.me_admission.map(|i| (i.votes, i.need)),
        Some((0, 2))
    );
    // and a refusal of another reason does not count
    let other = vec![(
        pid(2),
        Refusal {
            side: Side::Them,
            deny: Deny::Refused,
            at_unix: AT_UNIX,
        },
    )];
    assert!(!reply(&v, &other).me_pending);
}

#[test]
fn founder_is_not_pending() {
    // given: me alone, my own request on the list, and a refusal of mine against another node
    let v = verdicts(&[], vec![(pid(1), request(&[], None))]);
    let mine = vec![(
        pid(2),
        Refusal {
            side: Side::Me,
            deny: Deny::Unlisted,
            at_unix: AT_UNIX,
        },
    )];
    // when / then: my row is a member without a request
    let r = reply(&v, &mine);
    let own = row_of(&r, pid(1));
    assert_eq!(own.status, "member");
    assert!(own.admission.is_none());
    assert!(!r.me_pending);
}

#[test]
fn counts_pending_apart_from_unlisted() {
    // given: one request and one node known only from a refusal
    let (x, ghost) = (pid(9), pid(8));
    let v = verdicts(&[pid(2)], vec![(x, request(&[pid(2)], None))]);
    let refusals = vec![(
        ghost,
        Refusal {
            side: Side::Me,
            deny: Deny::Unlisted,
            at_unix: AT_UNIX,
        },
    )];
    // when / then
    let c = reply(&v, &refusals).counts;
    assert_eq!((c.members, c.pending, c.unlisted), (2, 1, 1));
}

#[test]
fn vote_summary_counts_admitting_and_me_pending() {
    // given: two requests, one of them already confirmed by me, and me waiting too
    let (x, y) = (pid(9), pid(8));
    let v = verdicts(
        &[pid(2)],
        vec![
            (x, request(&[pid(2)], None)),
            (y, request(&[pid(1)], None)),
            (pid(1), request(&[], None)),
        ],
    );
    // when
    let s = vote_summary(&reply(&v, &unlisted_by_them()));
    // then: only x awaits my confirmation
    assert_eq!((s.admitting, s.me_pending, s.voting), (1, true, 0));
}

#[test]
fn pending_sorts_after_members() {
    // given: a member, a request, an unlisted node and an excluded one
    let (a, x, ghost) = (pid(2), pid(9), pid(8));
    let mut v = verdicts(&[a], vec![(x, request(&[a], None))]);
    let mut ledger = (*v.ledger).clone();
    ledger.excluded.insert(
        pid(7),
        crate::members::access::tally::Exclusion {
            how: How::Votes,
            at_micros: 1,
            voters: vec![a],
        },
    );
    v.ledger = Arc::new(ledger);
    let refusals = vec![(
        ghost,
        Refusal {
            side: Side::Me,
            deny: Deny::Unlisted,
            at_unix: AT_UNIX,
        },
    )];
    // when / then
    let order: Vec<&str> = reply(&v, &refusals)
        .members
        .iter()
        .map(|r| r.status)
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        ["member", "member", "pending", "unlisted", "excluded"]
    );
}
