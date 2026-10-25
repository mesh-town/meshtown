//! Tests of the verdict view (`view.rs`), part four: a node that was excluded by votes keeps judging its
//! neighbours by the ordinary rules, while a node that left on its own refuses everyone. Child module of
//! `view_tests.rs`: `cast`, `ledger`, `verdicts`, `view_of` and the other helpers are there.

use super::*;
use crate::members::access::tally::ForgetRound;

mod me_excluded_by_votes {
    use super::*;

    #[test]
    fn listed_peer_is_allowed() {
        // given: I am excluded by votes, A is a member
        let Cast { m, a, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[a], &[m])));
        // when / then
        assert_eq!(view.check(a), Ok(()));
        assert!(view.allows(a));
    }

    #[test]
    fn excluded_peer_is_still_excluded() {
        // given: I and B are excluded by votes
        let Cast { m, a, b, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[a], &[m, b])));
        // when / then
        assert_eq!(view.check(b), Err(Deny::Excluded));
    }

    #[test]
    fn unknown_peer_is_unlisted() {
        // given: I am excluded by votes, X is not in the ledger
        let Cast { m, a, x, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[a], &[m])));
        // when / then
        assert_eq!(view.check(x), Err(Deny::Unlisted));
    }

    #[test]
    fn forgotten_peer_is_still_forgotten() {
        // given: I am excluded by votes, C was removed from the network
        let Cast { m, a, c, .. } = cast();
        let mut forgotten = ledger(&[a, c], &[m]);
        let round = ForgetRound {
            voters: Vec::new(),
            at_unix: Some(5),
        };
        forgotten.forgets.insert(c, round);
        let view = view_of(verdicts(m, forgotten));
        // when / then
        assert_eq!(view.check(c), Err(Deny::Forgotten));
    }

    #[test]
    fn my_own_votes_and_blocks_still_count() {
        // given: I am excluded by votes, I voted against C and blocked D
        let Cast { m, a, c, d, .. } = cast();
        let view = view_of(Verdicts {
            my_votes: set(&[c]),
            blocks: set(&[d]),
            ..verdicts(m, ledger(&[a, c, d], &[m]))
        });
        // when / then
        let got = outcomes(&view, &[a, c, d]);
        assert_eq!(got, vec![Ok(()), Err(Deny::VotedByMe), Err(Deny::Blocked)]);
    }
}

mod me_left_myself {
    use super::*;

    #[test]
    fn everyone_gets_left() {
        // given: I left the network, A is a member, X is a stranger
        let Cast { m, a, x, .. } = cast();
        let view = view_of(verdicts(m, ledger_left(&[a], &[], m)));
        // when / then
        assert_eq!(outcomes(&view, &[a, x]), vec![Err(Deny::Left); 2]);
    }
}

mod on_publish {
    use super::*;

    #[test]
    fn excluding_me_by_votes_denies_no_listed_node() {
        // given: a view without exclusions, A and B are members
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::open();
        view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        // when: I get excluded by votes
        let got = view.publish(verdicts(m, ledger(&[a, b], &[m])));
        // then: nobody is newly denied
        assert!(got.is_empty(), "got {got:?}");
        assert_eq!(outcomes(&view, &[a, b]), vec![Ok(()); 2]);
    }

    #[test]
    fn withdrawn_vote_exclusion_of_me_wakes_nobody() {
        // given: I am excluded by votes and a subscriber waits for allowed nodes
        let Cast { m, a, b, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[a, b], &[m])));
        let rx = view.allowed_changes();
        // when: my exclusion is withdrawn
        view.publish(verdicts(m, ledger(&[m, a, b], &[])));
        // then: nobody was refused before, so nobody is newly allowed
        let woken = rx.has_changed().expect("the view keeps the sender");
        assert!(!woken, "neighbours were allowed all along");
    }
}

mod ledger_query {
    use super::*;

    #[test]
    fn how_excluded_reports_the_way_out() {
        // given: A is excluded by votes, B left on its own, C is a member
        let Cast { a, b, c, .. } = cast();
        let l = ledger_left(&[c], &[a], b);
        // when / then
        assert_eq!(l.how_excluded(&a), Some(How::Votes));
        assert_eq!(l.how_excluded(&b), Some(How::Left));
        assert_eq!(l.how_excluded(&c), None);
    }
}
