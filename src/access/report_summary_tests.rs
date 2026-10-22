//! Tests of `vote_summary` (`report.rs`): only the rows that wait for this node's confirmation are counted.
//! The reply is built by hand; `me` is the node with seed 1.

use super::*;
use crate::net::contract::pid;
use crate::web::{Counts, MemberRow, MembersReply, VoteInfo};

fn vote(proposed_by: PeerId, mine: bool) -> VoteInfo {
    VoteInfo { votes: 1, need: 2, proposed_by: proposed_by.to_string(), voters: vec![], mine, reason: None }
}

fn row(id: PeerId, status: &'static str, vote: Option<VoteInfo>) -> MemberRow {
    MemberRow {
        id: String::new(),
        id_full: id.to_string(),
        name: None,
        is_self: id == pid(1),
        online: true,
        status,
        vote,
        excluded: None,
        blocked_by_me: false,
        denied_by_me: None,
        last_refusal: None,
        admission: None,
    }
}

fn reply(rows: Vec<MemberRow>, me_excluded: bool) -> MembersReply {
    let counts = Counts { members: 0, voting: 0, excluded: 0, unlisted: 0, pending: 0 };
    MembersReply { me: pid(1).to_string(), me_excluded, me_pending: false, me_admission: None, need: 2, counts, members: rows, changed: None }
}

fn voting(rows: Vec<MemberRow>) -> u64 {
    vote_summary(&reply(rows, false)).voting
}

#[test]
fn counts_a_pending_vote_of_another_node() {
    // given: a listed node with a vote that someone else proposed
    let rows = vec![row(pid(2), "member", Some(vote(pid(3), false)))];
    // when / then: it waits for me
    assert_eq!(voting(rows), 1);
}

#[test]
fn skips_a_vote_that_is_mine() {
    assert_eq!(voting(vec![row(pid(2), "member", Some(vote(pid(3), true)))]), 0);
}

#[test]
fn skips_a_vote_proposed_by_me() {
    assert_eq!(voting(vec![row(pid(2), "member", Some(vote(pid(1), false)))]), 0);
}

#[test]
fn skips_excluded_and_unlisted_rows() {
    let rows = vec![
        row(pid(2), "excluded", Some(vote(pid(3), false))),
        row(pid(4), "unlisted", Some(vote(pid(3), false))),
    ];
    assert_eq!(voting(rows), 0);
}

#[test]
fn skips_my_own_row() {
    assert_eq!(voting(vec![row(pid(1), "member", Some(vote(pid(3), false)))]), 0);
}

#[test]
fn empty_list_counts_nothing_and_carries_the_exclusion_flag() {
    assert_eq!(vote_summary(&reply(vec![], false)), VoteSummary { voting: 0, me_excluded: false, admitting: 0, me_pending: false });
    assert_eq!(vote_summary(&reply(vec![], true)), VoteSummary { voting: 0, me_excluded: true, admitting: 0, me_pending: false });
}
