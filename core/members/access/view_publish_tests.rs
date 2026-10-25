//! Tests of the verdict view (`view.rs`), part two: what a publication reports as newly denied, the refusal
//! log and the close reasons on the wire. Child module of `view_tests.rs`: `cast`, `verdicts`, `ledger` and the
//! other helpers are there.

use super::*;

mod publish {
    use super::*;

    #[test]
    fn publish_returns_only_newly_denied() {
        // given: a view whose first publication excludes B
        let Cast { m, a, b, c, .. } = cast();
        let view = AccessView::pending();
        let first = view.publish(verdicts(m, ledger(&[m, a, c], &[b])));
        assert_eq!(
            denied(first),
            map(&[(b, Deny::Excluded)]),
            "first publication"
        );
        // when: C gets blocked as well while B stays excluded
        let second = view.publish(Verdicts {
            blocks: set(&[c]),
            ..verdicts(m, ledger(&[m, a, c], &[b]))
        });
        // then: only C is new
        assert_eq!(
            denied(second),
            map(&[(c, Deny::Blocked)]),
            "second publication"
        );
        // when: the very same verdicts are published again
        let third = view.publish(Verdicts {
            blocks: set(&[c]),
            ..verdicts(m, ledger(&[m, a, c], &[b]))
        });
        // then: nothing is new
        assert!(third.is_empty(), "nothing changed, yet got {third:?}");
    }

    #[test]
    fn publish_returns_nothing_when_nobody_is_denied() {
        // given: a view
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::open();
        // when: verdicts that deny nobody are published
        let got = view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        // then: the result is empty
        assert!(got.is_empty(), "got {got:?}");
    }

    #[test]
    fn publish_returns_a_node_again_when_its_denial_comes_back() {
        // given: B was excluded and then listed again
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::pending();
        view.publish(verdicts(m, ledger(&[m, a], &[b])));
        let relisted = view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        assert!(
            relisted.is_empty(),
            "relisting denies nobody, got {relisted:?}"
        );
        // when: B is excluded once more
        let again = view.publish(verdicts(m, ledger(&[m, a], &[b])));
        // then: it counts as newly denied
        assert_eq!(denied(again), map(&[(b, Deny::Excluded)]));
    }

    #[test]
    fn publish_reports_my_vote_on_a_listed_member() {
        // given: A and B are listed and allowed
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::open();
        view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        // when: my vote against A is published (the threshold is not reached, A stays listed)
        let got = view.publish(Verdicts {
            my_votes: set(&[a]),
            ..verdicts(m, ledger(&[m, a, b], &[]))
        });
        // then: A is newly denied as "voted by me"
        assert_eq!(denied(got), map(&[(a, Deny::VotedByMe)]));
    }

    #[test]
    fn publish_returns_a_node_that_dropped_off_the_list() {
        // A node that is in neither ledger set of the new view (a cascade removed it) is reported as well.
        // given: D was listed in the previous view
        let Cast { m, a, d, .. } = cast();
        let view = AccessView::pending();
        view.publish(verdicts(m, ledger(&[m, a, d], &[])));
        // when: the new ledger no longer lists D
        let got = view.publish(verdicts(m, ledger(&[m, a], &[])));
        // then: D is newly denied as unlisted
        assert_eq!(denied(got), map(&[(d, Deny::Unlisted)]));
    }

    #[test]
    fn excluding_me_denies_every_listed_node_with_left() {
        // given: A and B are listed and allowed
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::open();
        view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        // when: I leave the network
        let got = view.publish(verdicts(m, ledger_left(&[a, b], &[], m)));
        // then: everybody else is newly denied with "left", I am not in the list
        assert_eq!(denied(got), map(&[(a, Deny::Left), (b, Deny::Left)]));
    }
}

mod refusals {
    use super::*;

    /// A view that does not list A, with my refusal of A already noted.
    fn refused_unlisted() -> (AccessView, Cast) {
        let cast = cast();
        let view = AccessView::pending();
        view.publish(verdicts(cast.m, ledger(&[cast.m], &[])));
        view.note_refusal(cast.a, Side::Me, Deny::Unlisted);
        (view, cast)
    }

    #[test]
    fn publish_drops_my_refusal_once_the_denial_is_lifted() {
        // given: my refusal of an unlisted node
        let (view, Cast { m, a, .. }) = refused_unlisted();
        // when: the node gets listed
        view.publish(verdicts(m, ledger(&[m, a], &[])));
        // then: the refusal is gone
        assert!(view.refusals().is_empty(), "{:?}", view.refusals());
    }

    #[test]
    fn publish_keeps_my_refusal_while_the_same_denial_stands() {
        // given: my refusal of an unlisted node
        let (view, Cast { m, a, .. }) = refused_unlisted();
        // when: a new publication still leaves the node unlisted
        view.publish(verdicts(m, ledger(&[m], &[])));
        // then: the refusal stays
        assert_eq!(view.refusals().len(), 1);
        assert_eq!(view.refusals()[0].1.deny, Deny::Unlisted);
        assert_eq!(view.refusals()[0].0, a);
    }

    #[test]
    fn publish_drops_my_refusal_whose_reason_changed() {
        // given: my refusal of an unlisted node
        let (view, Cast { m, a, .. }) = refused_unlisted();
        // when: the node gets listed and excluded: the denial has another reason now
        view.publish(verdicts(m, ledger(&[m, a], &[a])));
        // then: the old refusal is not kept
        assert!(view.refusals().is_empty(), "{:?}", view.refusals());
    }

    #[test]
    fn publish_keeps_their_refusal() {
        // given: a refusal reported by the other side
        let Cast { m, a, .. } = cast();
        let view = AccessView::pending();
        view.note_refusal(a, Side::Them, Deny::Excluded);
        // when: the view is published and the node passes
        view.publish(verdicts(m, ledger(&[m, a], &[])));
        // then: the refusal stays
        assert_eq!(view.refusals().len(), 1);
    }

    #[test]
    fn clear_refusal_forgets_only_that_node() {
        // given: refusals for two nodes
        let Cast { a, b, .. } = cast();
        let view = AccessView::open();
        view.note_refusal(a, Side::Them, Deny::Excluded);
        view.note_refusal(b, Side::Me, Deny::Unlisted);
        // when: one is cleared
        view.clear_refusal(a);
        // then: the other one remains
        let left: Vec<PeerId> = view.refusals().iter().map(|(p, _)| *p).collect();
        assert_eq!(left, vec![b]);
    }

    #[test]
    fn refusals_keep_latest_per_peer_capped_at_64() {
        // given: a refusal by me and then a refusal by them for the same node A
        let Cast { a, .. } = cast();
        let view = AccessView::open();
        let before = now_unix();
        view.note_refusal(a, Side::Me, Deny::Unlisted);
        view.note_refusal(a, Side::Them, Deny::Excluded);
        // then: one entry for A, the latest one, stamped with the current time
        let list = view.refusals();
        assert_eq!(list.len(), 1, "one entry per node, got {list:?}");
        let (peer, refusal) = &list[0];
        assert_eq!(*peer, a);
        assert_eq!(refusal.side, Side::Them);
        assert_eq!(refusal.deny, Deny::Excluded);
        assert!(
            refusal.at_unix >= before && refusal.at_unix <= now_unix() + 1,
            "at_unix {}",
            refusal.at_unix
        );
        // when: 100 other nodes are refused
        let others: Vec<PeerId> = (10..=109u8).map(pid).collect();
        others
            .iter()
            .for_each(|p| view.note_refusal(*p, Side::Me, Deny::Unlisted));
        // then: no more than 64 entries remain, one per node, and the latest node is among them
        let list = view.refusals();
        let distinct: BTreeSet<PeerId> = list.iter().map(|(p, _)| *p).collect();
        assert_eq!(list.len(), 64, "the log is capped at 64");
        assert_eq!(distinct.len(), 64, "one entry per node");
        assert!(
            distinct.contains(others.last().unwrap()),
            "the latest refusal must be kept"
        );
    }

    #[test]
    fn oldest_refusal_goes_first_over_the_cap() {
        // given: 64 refused nodes with seeds 1..=64, the first one is the oldest
        let view = AccessView::open();
        (1..=64u8).for_each(|seed| view.note_refusal(pid(seed), Side::Me, Deny::Unlisted));
        assert_eq!(view.refusals().len(), 64);
        // when: one more node is refused
        view.note_refusal(pid(65), Side::Me, Deny::Unlisted);
        // then: the oldest is gone, the newest is there, the size stays 64
        let peers: BTreeSet<PeerId> = view.refusals().iter().map(|(p, _)| *p).collect();
        assert_eq!(peers.len(), 64);
        assert!(!peers.contains(&pid(1)), "the oldest refusal must go");
        assert!(peers.contains(&pid(65)), "the newest refusal must stay");
    }

    #[test]
    fn refusals_are_shared_between_clones_of_the_view() {
        // given: a clone of the view
        let Cast { a, .. } = cast();
        let view = AccessView::open();
        let clone = view.clone();
        // when: a refusal is noted through the clone
        clone.note_refusal(a, Side::Me, Deny::Blocked);
        // then: the original sees it
        assert_eq!(view.refusals().len(), 1);
    }
}

mod wire {
    use super::*;

    #[test]
    fn wire_reason_roundtrips_through_from_wire() {
        // given: the reasons that survive the wire as they are
        for deny in [Deny::Left, Deny::Excluded, Deny::Unlisted, Deny::Refused] {
            // when: the close text the other side sees is parsed
            let text = on_the_wire(deny);
            // then: the same denial comes back
            assert_eq!(Deny::from_wire(&text), Some(deny), "text {text:?}");
        }
    }

    #[test]
    fn my_vote_and_block_reach_the_other_side_as_refused() {
        // given: the two reasons that stay private
        for deny in [Deny::VotedByMe, Deny::Blocked] {
            // when / then: the other side only learns that it was refused
            let text = on_the_wire(deny);
            assert_eq!(Deny::from_wire(&text), Some(Deny::Refused), "text {text:?}");
        }
    }

    #[test]
    fn wire_reasons_are_the_documented_words() {
        // given / when / then: the exact bytes older nodes also see
        assert_eq!(Deny::Unlisted.wire_reason(), b"not admitted");
        assert_eq!(Deny::Excluded.wire_reason(), b"excluded");
        assert_eq!(Deny::Left.wire_reason(), b"left");
        assert_eq!(Deny::VotedByMe.wire_reason(), b"refused");
        assert_eq!(Deny::Blocked.wire_reason(), b"refused");
        assert_eq!(Deny::Refused.wire_reason(), b"refused");
        assert_eq!(ACCESS_CLOSE_CODE, 403);
    }

    #[test]
    fn other_close_codes_and_odd_text_are_not_denials() {
        // given: close texts that are not a list refusal
        let texts = [
            "",
            "code 401: bad secret",
            "code 401: excluded",
            "code 408: timeout",
            "code 0: ",
            "code 403: ",
            "code 403: something else",
            "excluded",
        ];
        // when / then: none of them parses
        for text in texts {
            assert_eq!(Deny::from_wire(text), None, "text {text:?}");
        }
    }

    #[test]
    fn code_words_match_the_json_contract() {
        // given / when / then
        assert_eq!(Deny::Left.code(), "left");
        assert_eq!(Deny::Excluded.code(), "excluded");
        assert_eq!(Deny::VotedByMe.code(), "voted_by_me");
        assert_eq!(Deny::Blocked.code(), "blocked");
        assert_eq!(Deny::Unlisted.code(), "unlisted");
        assert_eq!(Deny::Refused.code(), "refused");
    }
}
