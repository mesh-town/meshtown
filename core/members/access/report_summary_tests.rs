//! Tests of `vote_summary` (`report.rs`): only the pending rows of other nodes are counted as waiting.
//! The reply is built by hand; `me` is the node with seed 1.

use super::*;
use crate::transport::net::contract::pid;
use crate::web_gui::{Counts, MemberRow, MembersReply};

fn row(id: PeerId, status: &'static str) -> MemberRow {
    MemberRow {
        id: String::new(),
        id_full: id.to_string(),
        name: None,
        is_self: id == pid(1),
        online: true,
        admin: false,
        status,
        excluded: None,
        blocked_by_me: false,
        denied_by_me: None,
        last_seen_sec: None,
        last_refusal: None,
        admission: None,
    }
}

fn reply(rows: Vec<MemberRow>, me_excluded: bool) -> MembersReply {
    let counts = Counts {
        members: 0,
        excluded: 0,
        unlisted: 0,
        pending: 0,
    };
    MembersReply {
        me: pid(1).to_string(),
        me_excluded,
        me_pending: false,
        admins: Vec::new(),
        me_admin: false,
        root: None,
        me_admission: None,
        counts,
        members: rows,
        changed: None,
    }
}

fn admitting(rows: Vec<MemberRow>) -> u64 {
    vote_summary(&reply(rows, false)).admitting
}

#[test]
fn counts_a_request_of_another_node() {
    assert_eq!(admitting(vec![row(pid(2), "pending")]), 1);
}

#[test]
fn skips_members_excluded_and_unlisted_rows() {
    let rows = vec![
        row(pid(2), "member"),
        row(pid(3), "excluded"),
        row(pid(4), "unlisted"),
    ];
    assert_eq!(admitting(rows), 0);
}

#[test]
fn skips_my_own_row() {
    assert_eq!(admitting(vec![row(pid(1), "pending")]), 0);
}

#[test]
fn empty_list_counts_nothing_and_carries_the_exclusion_flag() {
    assert_eq!(
        vote_summary(&reply(vec![], false)),
        VoteSummary {
            me_excluded: false,
            admitting: 0,
            me_pending: false
        }
    );
    assert_eq!(
        vote_summary(&reply(vec![], true)),
        VoteSummary {
            me_excluded: true,
            admitting: 0,
            me_pending: false
        }
    );
}
