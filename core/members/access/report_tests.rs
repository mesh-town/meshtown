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

mod order {
    use super::*;

    #[test]
    fn self_row_first_then_members_by_name() {
        // given: me, two named members, a nameless member, an unlisted named node and an excluded one
        let (me, zeta, alpha, nameless, ghost, gone) =
            (pid(1), pid(2), pid(3), pid(4), pid(5), pid(6));
        let w = world(&[zeta, alpha, nameless], &[gone]);
        w.members.set_name(zeta, "zeta".into());
        w.members.set_name(alpha, "alpha".into());
        w.members.set_name(ghost, "ghost".into());
        w.members.set_name(me, "laptop".into());
        // when: the reply is built
        let reply = w.reply();
        // then: me, members by name (nameless last), unlisted, excluded
        let ids: Vec<&str> = reply.members.iter().map(|r| r.id_full.as_str()).collect();
        let want: Vec<String> = [me, alpha, zeta, nameless, ghost, gone]
            .iter()
            .map(|p| p.to_string())
            .collect();
        assert_eq!(ids, want.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(
            reply.members.iter().map(|r| r.status).collect::<Vec<_>>(),
            ["member", "member", "member", "member", "unlisted", "excluded"]
        );
    }

    #[test]
    fn nameless_rows_of_one_kind_go_by_id() {
        // given: two nameless members
        let (b, c) = (pid(3), pid(2));
        let w = world(&[b, c], &[]);
        // when / then: after my row they are ordered by the full id
        let ids: Vec<String> = w
            .reply()
            .members
            .iter()
            .skip(1)
            .map(|r| r.id_full.clone())
            .collect();
        let mut want = vec![b.to_string(), c.to_string()];
        want.sort();
        assert_eq!(ids, want);
    }

    #[test]
    fn my_row_is_first_even_when_excluded() {
        // given: I have left; a is listed
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.exclude(pid(1), How::Left, vec![pid(1)]);
        // when / then: my row is the first one and the reply says so
        let reply = w.reply();
        assert!(reply.me_excluded);
        assert!(reply.members[0].is_self, "my row must come first");
        assert_eq!(reply.members[0].status, "excluded");
    }
}

mod fields {
    use super::*;

    #[test]
    fn pending_vote_fields_match_contract() {
        // given: four members; b has proposed to exclude a: one vote of two
        let (a, b) = (pid(2), pid(3));
        let mut w = world(&[a, b, pid(4)], &[]);
        w.pending(a, vec![b], 2);
        // when
        let row = w.row(a);
        // then: the contract fields of the voting
        let vote = row.vote.expect("a vote in progress");
        assert_eq!((vote.votes, vote.need), (1, 2));
        assert_eq!(vote.proposed_by, b.to_string());
        assert_eq!(vote.voters, vec![b.to_string()]);
        assert!(!vote.mine, "my vote is not among them");
        assert_eq!(
            row.status, "member",
            "a pending vote does not change the status"
        );
    }

    #[test]
    fn vote_reason_comes_from_the_verdicts() {
        // given: b proposed to exclude a and a reason is known for a; c has a vote without a reason
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b, c, pid(5)], &[]);
        w.pending(a, vec![b], 2);
        w.pending(c, vec![b], 2);
        w.verdicts.reasons.insert(a, "noisy link".to_string());
        // when / then: the reason shows for a only
        assert_eq!(
            w.row(a).vote.expect("a vote in progress").reason.as_deref(),
            Some("noisy link")
        );
        assert_eq!(w.row(c).vote.expect("a vote in progress").reason, None);
        let json = serde_json::to_value(w.row(c).vote).unwrap();
        assert!(
            json.get("reason").is_some_and(|r| r.is_null()),
            "the key must be present as null: {json}"
        );
    }

    #[test]
    fn my_vote_is_marked_mine_and_denies_locally() {
        // given: I proposed to exclude a
        let (a, me) = (pid(2), pid(1));
        let mut w = world(&[a, pid(3), pid(4)], &[]);
        w.pending(a, vec![me], 2);
        w.verdicts.my_votes.insert(a);
        // when
        let row = w.row(a);
        // then: marked as mine and refused at my side at once
        assert!(row.vote.expect("a vote in progress").mine);
        assert_eq!(row.denied_by_me, Some("voted_by_me"));
    }

    #[test]
    fn excluded_row_fields_match_contract() {
        // given: c was excluded by the votes of a and b
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b], &[c]);
        w.exclude(c, How::Votes, vec![a, b]);
        w.pending(c, vec![a], 2);
        // when
        let row = w.row(c);
        // then: the exclusion carries the contract fields; a vote is not shown for an excluded node
        let info = row.excluded.expect("exclusion info");
        assert_eq!((info.how, info.at_unix), ("votes", AT_UNIX));
        assert_eq!(info.voters, vec![a.to_string(), b.to_string()]);
        assert_eq!(row.status, "excluded");
        assert!(row.vote.is_none(), "a vote for an excluded node");
    }

    #[test]
    fn left_node_is_excluded_with_himself_as_voter() {
        // given: a has left
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.exclude(a, How::Left, vec![a]);
        // when / then
        let info = w.row(a).excluded.expect("exclusion info");
        assert_eq!((info.how, info.voters), ("left", vec![a.to_string()]));
    }

    #[test]
    fn online_follows_connections_and_my_row_is_online() {
        // given: a is connected, b is not
        let (a, b) = (pid(2), pid(3));
        let w = world(&[a, b], &[]);
        w.members.add_verified_conn(a);
        // when / then
        assert!(w.row(pid(1)).online, "my row");
        assert!(w.row(a).online, "connected");
        assert!(!w.row(b).online, "not connected");
    }

    #[test]
    fn blocked_node_not_on_the_list_shows_as_unlisted() {
        // given: I blocked a node that was never listed
        let x = pid(9);
        let mut w = world(&[], &[]);
        w.verdicts.blocks.insert(x);
        // when
        let row = w.row(x);
        // then
        assert_eq!((row.status, row.blocked_by_me), ("unlisted", true));
    }

    #[test]
    fn id_is_short_and_id_full_is_the_whole_key() {
        let w = world(&[pid(2)], &[]);
        let row = w.row(pid(2));
        assert_eq!(row.id_full, pid(2).to_string());
        assert!(
            row.id_full.starts_with(&row.id) && row.id.len() < row.id_full.len(),
            "short id: {}",
            row.id
        );
    }
}

mod denied_by_me_codes {
    use super::*;

    #[test]
    fn denied_by_me_codes() {
        // given: a is a member, b is excluded, c is blocked, d is unlisted, e has my vote, f is excluded and blocked
        let (a, b, c, d, e, f) = (pid(2), pid(3), pid(4), pid(5), pid(6), pid(7));
        let mut w = world(&[a, c, e], &[b, f]);
        w.verdicts.blocks.extend([c, f]);
        w.verdicts.my_votes.insert(e);
        w.members.set_name(d, "stranger".into());
        // when / then: the codes follow the order of the check
        let codes: Vec<_> = [pid(1), a, b, c, d, e, f]
            .iter()
            .map(|p| w.row(*p).denied_by_me)
            .collect();
        assert_eq!(
            codes,
            [
                None,
                None,
                Some("excluded"),
                Some("blocked"),
                Some("unlisted"),
                Some("voted_by_me"),
                Some("excluded")
            ]
        );
    }

    #[test]
    fn after_leaving_everyone_is_denied_and_my_row_is_not() {
        // given: I have left
        let mut w = world(&[pid(2)], &[]);
        w.exclude(pid(1), How::Left, vec![pid(1)]);
        // when / then
        assert_eq!(w.row(pid(2)).denied_by_me, Some("left"));
        assert_eq!(w.row(pid(1)).denied_by_me, None, "my own row");
    }

    #[test]
    fn unlisted_node_is_denied_as_unlisted() {
        // given: a node the list does not know
        let x = pid(9);
        let w = world(&[], &[]);
        w.members.set_name(x, "newcomer".into());
        // when / then
        let row = w.row(x);
        assert_eq!(
            (row.status, row.denied_by_me),
            ("unlisted", Some("unlisted"))
        );
    }
}

mod refusals {
    use super::*;

    #[test]
    fn refusal_from_them_is_reported() {
        // given: a refused me at the handshake with his vote or block
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.refusal(a, Side::Them, Deny::Refused);
        // when
        let info = w.row(a).last_refusal.expect("a refusal");
        // then
        assert_eq!(
            (info.side, info.reason, info.at_unix),
            ("them", "refused", AT_UNIX + 100)
        );
    }

    #[test]
    fn refusal_by_me_names_my_reason() {
        let a = pid(2);
        let mut w = world(&[], &[a]);
        w.refusal(a, Side::Me, Deny::Excluded);
        let info = w.row(a).last_refusal.expect("a refusal");
        assert_eq!((info.side, info.reason), ("me", "excluded"));
    }

    #[test]
    fn refusal_of_an_unknown_node_makes_a_row() {
        // given: a refusal of a node nobody knows
        let x = pid(9);
        let mut w = world(&[], &[]);
        w.refusal(x, Side::Me, Deny::Unlisted);
        // when / then: the page shows it as unlisted, and it is counted
        let reply = w.reply();
        assert!(reply
            .members
            .iter()
            .any(|r| r.id_full == x.to_string() && r.status == "unlisted"));
        assert_eq!(reply.counts.unlisted, 1);
    }
}

mod totals {
    use super::*;

    #[test]
    fn counts_follow_the_rows() {
        // given: me and a, b, c listed, d excluded, e unlisted; a has a pending vote
        let (a, b, c, d, e) = (pid(2), pid(3), pid(4), pid(5), pid(6));
        let mut w = world(&[a, b, c], &[d]);
        w.members.set_name(e, "stranger".into());
        w.pending(a, vec![b], 2);
        // when
        let counts = w.reply().counts;
        // then: members include me
        assert_eq!(
            (
                counts.members,
                counts.voting,
                counts.excluded,
                counts.unlisted
            ),
            (4, 1, 1, 1)
        );
    }

    #[test]
    fn need_and_changed_are_passed_through() {
        // given: a list that needs one vote
        let mut w = world(&[pid(2)], &[]);
        w.ledger.need = 1;
        // when / then
        assert_eq!(w.reply().need, 1);
        assert_eq!(w.reply().changed, None);
        assert_eq!(
            members_reply(&w.verdicts(), &w.members, &[], Some(false)).changed,
            Some(false)
        );
        assert_eq!(
            members_reply(&w.verdicts(), &w.members, &[], Some(true)).changed,
            Some(true)
        );
    }

    #[test]
    fn reply_names_me_by_full_id() {
        let w = world(&[], &[]);
        assert_eq!(w.reply().me, pid(1).to_string());
    }
}

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
