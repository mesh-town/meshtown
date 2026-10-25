//! Tests of the verdict view (`view.rs`), part three: the "allowed again" counter that wakes the dialer when a
//! publication lets in a node the previous view refused. Child module of `view_tests.rs`: `cast`, `verdicts`,
//! `ledger`, `view_of` and the other helpers are there.

use super::*;

mod publish_over_a_previous_view {
    use super::*;

    mod denial_lifted {
        use super::*;

        #[test]
        fn withdrawn_exclusion_wakes() {
            // given: a view where B is excluded, and a subscriber
            let Cast { m, a, b, .. } = cast();
            let (view, mut rx) = watched(verdicts(m, ledger(&[m, a], &[b])));
            assert_eq!(view.check(b), Err(Deny::Excluded));
            // when: the same view is published without the exclusion
            let denied_now = view.publish(verdicts(m, ledger(&[m, a, b], &[])));
            // then: the subscriber is told, B is let in and nobody is newly denied
            assert!(changed(&mut rx), "a withdrawn exclusion must wake");
            assert_eq!(view.check(b), Ok(()));
            assert!(denied_now.is_empty(), "got {denied_now:?}");
        }

        #[test]
        fn withdrawn_own_exclusion_wakes_for_every_known_peer() {
            // given: I left the network, so everybody I know is refused as `Left`
            let Cast { m, a, b, .. } = cast();
            let (view, mut rx) = watched(verdicts(m, ledger_left(&[a, b], &[], m)));
            assert_eq!(view.check(a), Err(Deny::Left));
            assert_eq!(view.check(b), Err(Deny::Left));
            // when: my exclusion is gone
            view.publish(verdicts(m, ledger(&[m, a, b], &[])));
            // then: one wake, and both are let in
            assert!(changed(&mut rx), "a withdrawn own exclusion must wake");
            assert_eq!(
                outcomes(&view, &[a, b]),
                vec![Ok(()); 2],
                "results for A, B"
            );
        }

        #[test]
        fn every_lifted_deny_wakes() {
            // given: one case per refusal that a publication can lift
            let Cast { m, a, .. } = cast();
            let listed = || ledger(&[m, a], &[]);
            let cases = [
                (
                    "voted by me",
                    Verdicts {
                        my_votes: set(&[a]),
                        ..verdicts(m, listed())
                    },
                    Deny::VotedByMe,
                ),
                (
                    "blocked",
                    Verdicts {
                        blocks: set(&[a]),
                        ..verdicts(m, listed())
                    },
                    Deny::Blocked,
                ),
                ("unlisted", verdicts(m, ledger(&[m], &[])), Deny::Unlisted),
            ];
            for (case, before, deny) in cases {
                let (view, mut rx) = watched(before);
                assert_eq!(view.check(a), Err(deny), "{case}: the setup must refuse A");
                // when: the refusal is gone
                view.publish(verdicts(m, listed()));
                // then: the subscriber is told
                assert!(changed(&mut rx), "{case}: a lifted refusal must wake");
                assert_eq!(view.check(a), Ok(()), "{case}");
            }
        }
    }

    mod nobody_newly_allowed {
        use super::*;

        #[test]
        fn first_publish_does_not_wake() {
            // given: two pending views and a subscriber on each
            let Cast { m, a, b, .. } = cast();
            let (denying, mut denying_rx) = pending_watched();
            let (open, mut open_rx) = pending_watched();
            // when: the first publication of each arrives, one with a refusal and one without
            denying.publish(verdicts(m, ledger(&[m, a], &[b])));
            open.publish(verdicts(m, ledger(&[m, a, b], &[])));
            // then: nobody is woken: until then the view let everybody in anyway
            assert!(
                !changed(&mut denying_rx),
                "first publication with a refusal"
            );
            assert!(
                !changed(&mut open_rx),
                "first publication without a refusal"
            );
        }

        #[test]
        fn unchanged_verdicts_do_not_wake() {
            // given: a view with B excluded, and a subscriber
            let Cast { m, a, b, .. } = cast();
            let v = verdicts(m, ledger(&[m, a], &[b]));
            let (view, mut rx) = watched(v.clone());
            // when: the very same verdicts are published again
            view.publish(v);
            // then: nothing is woken
            assert!(!changed(&mut rx), "an unchanged view must not wake");
        }

        #[test]
        fn new_exclusion_does_not_wake() {
            // given: a view where B is let in, and a subscriber
            let Cast { m, a, b, .. } = cast();
            let (view, mut rx) = watched(verdicts(m, ledger(&[m, a, b], &[])));
            // when: B gets excluded
            let denied_now = view.publish(verdicts(m, ledger(&[m, a], &[b])));
            // then: B is reported as newly denied and nobody is woken
            assert_eq!(denied(denied_now), map(&[(b, Deny::Excluded)]));
            assert!(!changed(&mut rx), "a new exclusion must not wake");
        }

        #[test]
        fn exclusion_withdrawn_behind_a_block_does_not_wake() {
            // given: B is excluded and also blocked by me
            let Cast { m, a, b, .. } = cast();
            let blocked = |l| Verdicts {
                blocks: set(&[b]),
                ..verdicts(m, l)
            };
            let (view, mut rx) = watched(blocked(ledger(&[m, a], &[b])));
            // when: the exclusion is withdrawn but the block stays
            view.publish(blocked(ledger(&[m, a, b], &[])));
            // then: B is still refused, now as blocked, and nobody is woken
            assert_eq!(view.check(b), Err(Deny::Blocked));
            assert!(!changed(&mut rx), "a block still holds B out");
        }
    }
}

// --- setup and helpers ---

/// An open view holding `first`, and a subscriber that has seen nothing yet.
fn watched(first: Verdicts) -> (AccessView, tokio::sync::watch::Receiver<u64>) {
    let view = view_of(first);
    let rx = view.allowed_changes();
    (view, rx)
}

/// A pending view with nothing published, and a subscriber.
fn pending_watched() -> (AccessView, tokio::sync::watch::Receiver<u64>) {
    let view = AccessView::pending();
    let rx = view.allowed_changes();
    (view, rx)
}

/// Whether the counter moved since the subscriber last looked.
fn changed(rx: &mut tokio::sync::watch::Receiver<u64>) -> bool {
    rx.has_changed().expect("the view keeps the sender")
}
