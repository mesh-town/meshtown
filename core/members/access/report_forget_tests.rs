//! Tests of the member list reply for removed ("forgotten") nodes: the `forgotten` status and the `forget` object,
//! a round that still waits for votes, `me_removed`, the counters, the row order and `last_seen_sec`.
//! The verdicts are built by hand, there is no replica here. `me` is the node with seed 1.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::members::access::report_forget::stamp_last_seen;
use crate::members::access::tally::{Exclusion, ForgetRound, How, Ledger};
use crate::transport::net::contract::pid;
use crate::web_gui::{MemberRow, MembersReply};

const AT_UNIX: u64 = 1_792_400_000;

mod forgotten_row {
    use super::*;

    #[test]
    fn has_status_and_info() {
        // given: a and b voted to remove c; I am not among the voters
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b, c]);
        w.round(c, &[a, b], Some(AT_UNIX));
        // when: the reply is built
        let row = w.row(c);
        // then: the row is forgotten and says who voted, since when, and that none of the votes is mine
        assert_eq!(row.status, "forgotten");
        let info = row
            .forget
            .expect("a forgotten row carries the forget object");
        assert_eq!((info.votes, info.need), (2, 2));
        assert_eq!(info.proposed_by, short(a));
        assert_eq!(info.voters, vec![short(a), short(b)]);
        assert_eq!(info.at_unix, Some(AT_UNIX));
        assert!(!info.mine, "I did not vote");
    }

    #[test]
    fn is_mine_when_i_voted() {
        // given: I proposed to remove c and a confirmed
        let (a, c) = (pid(2), pid(4));
        let mut w = world(&[a, c]);
        w.round(c, &[pid(1), a], Some(AT_UNIX));
        // when / then: the row is forgotten and the vote is marked as mine
        let row = w.row(c);
        assert_eq!(row.status, "forgotten");
        assert!(row.forget.expect("the forget object").mine);
        assert_eq!(row.denied_by_me, Some("removed"));
    }

    #[test]
    fn excluded_status_wins() {
        // given: c is excluded by votes and also has a finished removal round
        let (a, c) = (pid(2), pid(4));
        let mut w = world(&[a, c]);
        w.exclude(c);
        w.round(c, &[a], Some(AT_UNIX));
        // when / then: the status stays excluded
        assert_eq!(w.row(c).status, "excluded");
    }
}

mod waiting_round {
    use super::*;

    #[test]
    fn row_shows_votes_and_need() {
        // given: a proposed to remove c, the second vote is still missing
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b, c]);
        w.round(c, &[a], None);
        // when: the reply is built
        let row = w.row(c);
        // then: c stays a member, and the round shows one vote of two with no removal time
        assert_eq!(row.status, "member");
        let info = row.forget.expect("a live round carries the forget object");
        assert_eq!((info.votes, info.need), (1, 2));
        assert_eq!(info.proposed_by, short(a));
        assert_eq!(info.at_unix, None);
        assert!(!info.mine);
        assert_eq!(
            row.denied_by_me, None,
            "a vote of another hides nobody from me"
        );
    }

    #[test]
    fn proposer_row_has_denied_removed_by_me() {
        // given: I proposed to remove c and nobody confirmed yet
        let (a, c) = (pid(2), pid(4));
        let mut w = world(&[a, c]);
        w.round(c, &[pid(1)], None);
        // when / then: the row says the node is hidden by my own vote, and the round is mine
        let row = w.row(c);
        assert_eq!(row.denied_by_me, Some("removed_by_me"));
        assert!(row.forget.expect("the forget object").mine);
        assert_eq!(row.status, "member");
    }

    #[test]
    fn member_without_round_has_no_forget_info() {
        // given: a plain member
        let a = pid(2);
        let w = world(&[a]);
        // when / then: no forget object
        assert!(w.row(a).forget.is_none());
    }
}

mod me_removed {
    use super::*;

    #[test]
    fn false_when_nobody_removed_me() {
        // given: an ordinary list
        let w = world(&[pid(2)]);
        // then: I am not removed
        assert!(!w.reply().me_removed);
    }

    #[test]
    fn true_from_own_view() {
        // given: my own view has a finished round on my node
        let mut w = world(&[pid(2), pid(3)]);
        w.round(pid(1), &[pid(2), pid(3)], Some(AT_UNIX));
        // then: the reply says I am removed
        assert!(w.reply().me_removed);
    }

    #[test]
    fn true_from_them_refusal() {
        // given: another node turned me away as removed, and I have no verified connection
        let a = pid(2);
        let mut w = world(&[a]);
        w.refusal(a, Side::Them, Deny::Forgotten);
        // then: the reply says I am removed
        assert!(w.reply().me_removed);
    }

    #[test]
    fn false_when_connected() {
        // given: a refused me as removed, but I have a verified connection to b
        let (a, b) = (pid(2), pid(3));
        let mut w = world(&[a, b]);
        w.refusal(a, Side::Them, Deny::Forgotten);
        w.members.add_verified_conn(b);
        // then: I am still in the network of b, so I am not shown as removed
        assert!(!w.reply().me_removed);
    }

    #[test]
    fn false_for_a_refusal_made_by_me() {
        // given: I turned a node away as removed
        let a = pid(2);
        let mut w = world(&[a]);
        w.refusal(a, Side::Me, Deny::Forgotten);
        // then: that says nothing about me
        assert!(!w.reply().me_removed);
    }

    #[test]
    fn false_for_another_refusal_reason() {
        // given: another node turned me away as not admitted
        let a = pid(2);
        let mut w = world(&[a]);
        w.refusal(a, Side::Them, Deny::Unlisted);
        // then: I am not removed
        assert!(!w.reply().me_removed);
    }

    #[test]
    fn false_for_a_round_that_still_waits() {
        // given: a proposed to remove me, the circle is not complete
        let (a, b) = (pid(2), pid(3));
        let mut w = world(&[a, b]);
        w.round(pid(1), &[a], None);
        // then: the page of a removed node does not appear yet
        assert!(!w.reply().me_removed);
    }
}

mod order {
    use super::*;

    #[test]
    fn puts_forgotten_before_excluded() {
        // given: me, a member, an unlisted named node, a forgotten node and an excluded one
        let (member, ghost, forgotten, excluded) = (pid(2), pid(3), pid(4), pid(5));
        let mut w = world(&[member, forgotten, excluded]);
        w.members.set_name(ghost, "ghost".into());
        w.round(forgotten, &[member, pid(1)], Some(AT_UNIX));
        w.exclude(excluded);
        // when: the reply is built
        let rows = w.reply().members;
        // then: the order is me, member, unlisted, forgotten, excluded
        let ids: Vec<String> = rows.iter().map(|r| r.id_full.clone()).collect();
        let want: Vec<String> = [pid(1), member, ghost, forgotten, excluded]
            .iter()
            .map(|p| p.to_string())
            .collect();
        assert_eq!(ids, want);
        let statuses: Vec<&str> = rows.iter().map(|r| r.status).collect();
        assert_eq!(
            statuses,
            ["member", "member", "unlisted", "forgotten", "excluded"]
        );
    }
}

mod counts {
    use super::*;

    #[test]
    fn forgotten_are_counted_separately() {
        // given: me and a are members, c is forgotten, d has a round that waits, e is excluded
        let (a, c, d, e) = (pid(2), pid(4), pid(5), pid(6));
        let mut w = world(&[a, c, d, e]);
        w.round(c, &[a, pid(1)], Some(AT_UNIX));
        w.round(d, &[a], None);
        w.exclude(e);
        // when: the reply is built
        let counts = w.reply().counts;
        // then: the forgotten are not members; a waiting round does not make a node forgotten
        assert_eq!(counts.forgotten, 1);
        assert_eq!(counts.members, 3, "me, a and d");
        assert_eq!(counts.excluded, 1);
    }

    #[test]
    fn zero_forgotten_in_a_plain_list() {
        let w = world(&[pid(2)]);
        assert_eq!(w.reply().counts.forgotten, 0);
    }
}

mod last_seen {
    use super::*;

    #[test]
    fn comes_from_seen_map() {
        // given: a reply with two members and a map that knows the age of a only
        let (a, b) = (pid(2), pid(3));
        let w = world(&[a, b]);
        let mut reply = w.reply();
        let seen: BTreeMap<PeerId, u64> = [(a, 120)].into_iter().collect();
        // when: the ages are stamped
        stamp_last_seen(&mut reply, &seen);
        // then: a has its age, b has none
        assert_eq!(row_in(&reply, a).last_seen_sec, Some(120));
        assert_eq!(row_in(&reply, b).last_seen_sec, None);
    }

    #[test]
    fn stale_row_is_null_without_evidence() {
        // given: a reply nobody stamped, and a stamp with an empty map
        let a = pid(2);
        let w = world(&[a]);
        let mut reply = w.reply();
        // then: no row claims an age
        assert!(reply.members.iter().all(|r| r.last_seen_sec.is_none()));
        stamp_last_seen(&mut reply, &BTreeMap::new());
        assert!(
            reply.members.iter().all(|r| r.last_seen_sec.is_none()),
            "an empty map must not invent an age"
        );
    }
}

// --- setup and helpers ---

struct World {
    ledger: Ledger,
    members: Members,
    refusals: Vec<(PeerId, Refusal)>,
}

/// The node (seed 1) and `listed` are admitted by the list; no rounds, no exclusions, no connections.
fn world(listed: &[PeerId]) -> World {
    let admitted: BTreeSet<PeerId> = [&[pid(1)][..], listed].concat().into_iter().collect();
    let ledger = Ledger {
        admitted,
        excluded: BTreeMap::new(),
        pending: BTreeMap::new(),
        need: 2,
        admissions: BTreeMap::new(),
        forgets: BTreeMap::new(),
        admit_unix: BTreeMap::new(),
        forget_marks: BTreeMap::new(),
        forget_need: 2,
    };
    World {
        ledger,
        members: Members::default(),
        refusals: Vec::new(),
    }
}

impl World {
    /// A removal circle on `subject`; `at_unix` is set once the circle is complete.
    fn round(&mut self, subject: PeerId, voters: &[PeerId], at_unix: Option<u64>) {
        let round = ForgetRound {
            voters: voters.to_vec(),
            at_unix,
        };
        self.ledger.forgets.insert(subject, round);
    }

    fn exclude(&mut self, peer: PeerId) {
        let exclusion = Exclusion {
            how: How::Votes,
            at_micros: AT_UNIX * 1_000_000,
            voters: vec![pid(2)],
        };
        self.ledger.excluded.insert(peer, exclusion);
    }

    fn refusal(&mut self, peer: PeerId, side: Side, deny: Deny) {
        let at_unix = AT_UNIX + 100;
        let refusal = Refusal {
            side,
            deny,
            at_unix,
        };
        self.refusals.push((peer, refusal));
    }

    fn verdicts(&self) -> Verdicts {
        Verdicts {
            me: pid(1),
            ledger: Arc::new(self.ledger.clone()),
            my_votes: BTreeSet::new(),
            blocks: BTreeSet::new(),
            reasons: Default::default(),
        }
    }

    fn reply(&self) -> MembersReply {
        members_reply(&self.verdicts(), &self.members, &self.refusals, None)
    }

    fn row(&self, peer: PeerId) -> MemberRow {
        row_in(&self.reply(), peer)
    }
}

fn row_in(reply: &MembersReply, peer: PeerId) -> MemberRow {
    reply
        .members
        .iter()
        .find(|r| r.id_full == peer.to_string())
        .cloned()
        .expect("the row must exist")
}

fn short(p: PeerId) -> String {
    short_peer_id(&p)
}
