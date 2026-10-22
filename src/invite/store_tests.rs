//! Tests of the invite verdict and the issue record (`store.rs`).
//!
//! These are pure functions: `decide` and `issue_record`. The storage in the document
//! (`Ledger`) and the sweeper are covered by end-to-end loopback tests (`tests/invite_once_loopback.rs`).

use super::*;
use crate::invite::INVITE_TTL_SECS;

const NOW: u64 = 1_790_000_000;
const TTL: u64 = INVITE_TTL_SECS;
/// An issue whose record couldn't be read (the value is empty or the bytes didn't parse).
const UNREADABLE: Option<u64> = None;

mod decide_verdict {
    use super::*;

    mod used_stamp_present {
        use super::*;

        #[test]
        fn used_even_when_issue_is_fresh() {
            assert_eq!(decide(&[ago(10)], true, NOW), IssueVerdict::Used);
        }

        #[test]
        fn used_even_when_issue_is_expired() {
            assert_eq!(decide(&[ago(TTL + 100)], true, NOW), IssueVerdict::Used);
        }

        #[test]
        fn used_even_when_issue_is_unreadable() {
            assert_eq!(decide(&[UNREADABLE], true, NOW), IssueVerdict::Used);
        }

        #[test]
        fn used_even_when_issue_has_not_arrived_yet() {
            // The mark may arrive before the issue record: it must be redeemed anyway.
            assert_eq!(decide(&[], true, NOW), IssueVerdict::Used);
        }

        #[test]
        fn used_even_when_one_of_two_issues_is_fresh() {
            assert_eq!(
                decide(&[ago(TTL + 5), ago(10)], true, NOW),
                IssueVerdict::Used
            );
        }
    }

    mod no_stamp {
        use super::*;

        mod fresh_readable_issue {
            use super::*;

            #[test]
            fn grants() {
                assert_eq!(decide(&[ago(10)], false, NOW), IssueVerdict::Grant);
            }

            #[test]
            fn grants_one_second_before_the_ttl_ends() {
                assert_eq!(decide(&[ago(TTL - 1)], false, NOW), IssueVerdict::Grant);
            }

            #[test]
            fn grants_when_made_unix_is_ahead_of_our_clock() {
                // The issuer's clock ran ahead: an issue "from the future" is not expired.
                assert_eq!(decide(&[Some(NOW + 500)], false, NOW), IssueVerdict::Grant);
            }

            #[test]
            fn grants_when_our_clock_is_below_the_ttl() {
                // Computing `now - TTL` on a clock "from zero" must not overflow.
                assert_eq!(decide(&[Some(0)], false, 100), IssueVerdict::Grant);
            }

            #[test]
            fn grants_without_overflow_on_the_largest_made_unix() {
                // A record "from the future" counts as fresh.
                // Any member can write a record in the document, so `made_unix + TTL` must not
                // panic.
                assert_eq!(decide(&[Some(u64::MAX)], false, NOW), IssueVerdict::Grant);
            }
        }

        mod issue_at_the_ttl_boundary {
            use super::*;

            #[test]
            fn expired_exactly_at_made_unix_plus_ttl() {
                // The boundary is excluded: freshness is strictly `now < made_unix + TTL`.
                assert_eq!(decide(&[ago(TTL)], false, NOW), IssueVerdict::Expired);
            }

            #[test]
            fn expired_one_second_after_the_boundary() {
                assert_eq!(decide(&[ago(TTL + 1)], false, NOW), IssueVerdict::Expired);
            }

            #[test]
            fn expired_long_after() {
                assert_eq!(decide(&[ago(10 * TTL)], false, NOW), IssueVerdict::Expired);
            }
        }

        mod two_issues_from_two_authors {
            use super::*;

            #[test]
            fn expired_then_fresh_grants() {
                assert_eq!(
                    decide(&[ago(TTL + 100), ago(10)], false, NOW),
                    IssueVerdict::Grant
                );
            }

            #[test]
            fn fresh_then_expired_grants() {
                assert_eq!(
                    decide(&[ago(10), ago(TTL + 100)], false, NOW),
                    IssueVerdict::Grant
                );
            }

            #[test]
            fn both_expired_is_expired() {
                assert_eq!(
                    decide(&[ago(TTL + 100), ago(TTL)], false, NOW),
                    IssueVerdict::Expired
                );
            }
        }

        mod unreadable_issue {
            use super::*;

            #[test]
            fn alone_is_not_ready() {
                assert_eq!(decide(&[UNREADABLE], false, NOW), IssueVerdict::NotReady);
            }

            #[test]
            fn next_to_an_expired_one_is_not_ready() {
                // An unreadable issue may turn out fresh — we don't say "expired" conclusively.
                assert_eq!(
                    decide(&[UNREADABLE, ago(TTL + 100)], false, NOW),
                    IssueVerdict::NotReady
                );
                assert_eq!(
                    decide(&[ago(TTL + 100), UNREADABLE], false, NOW),
                    IssueVerdict::NotReady
                );
            }

            #[test]
            fn next_to_a_fresh_readable_one_still_grants() {
                assert_eq!(
                    decide(&[UNREADABLE, ago(10)], false, NOW),
                    IssueVerdict::Grant
                );
                assert_eq!(
                    decide(&[ago(10), UNREADABLE], false, NOW),
                    IssueVerdict::Grant
                );
            }
        }

        mod no_issues_at_all {
            use super::*;

            #[test]
            fn unknown() {
                assert_eq!(decide(&[], false, NOW), IssueVerdict::Unknown);
            }
        }
    }
}

mod record_for_whom {
    use super::*;

    #[test]
    fn limit_is_sixty_four_chars() {
        assert_eq!(FOR_WHOM_MAX_CHARS, 64);
    }

    #[test]
    fn keeps_made_unix_and_format_version() {
        let rec = issue_record(NOW, Some("Masha"));
        assert_eq!(rec.made_unix, NOW);
        assert_eq!(rec.v, 1);
    }

    #[test]
    fn no_name_stays_no_name() {
        assert_eq!(issue_record(NOW, None).for_whom, None);
    }

    #[test]
    fn short_name_is_kept_as_is() {
        let rec = issue_record(NOW, Some("Masha from work"));
        assert_eq!(rec.for_whom.as_deref(), Some("Masha from work"));
    }

    #[test]
    fn name_of_exactly_sixty_four_chars_is_kept() {
        let name = "ü".repeat(64);
        let rec = issue_record(NOW, Some(&name));
        assert_eq!(rec.for_whom.as_deref(), Some(name.as_str()));
    }

    #[test]
    fn longer_name_is_cut_to_sixty_four_chars_not_bytes() {
        // Latin letters like é are 2 bytes per character: cutting by bytes would give 32 characters or a split character.
        let name: String = "éøüåñæçîôû".chars().cycle().take(100).collect();
        let rec = issue_record(NOW, Some(&name));
        let want: String = name.chars().take(64).collect();
        assert_eq!(rec.for_whom.as_deref(), Some(want.as_str()));
    }

    #[test]
    fn name_of_sixty_five_chars_loses_only_the_last_one() {
        let name = format!("{}!", "ø".repeat(64));
        let rec = issue_record(NOW, Some(&name));
        assert_eq!(rec.for_whom.as_deref(), Some("ø".repeat(64).as_str()));
    }

    #[test]
    fn four_byte_chars_are_not_torn() {
        let name = "𝄞".repeat(70);
        let rec = issue_record(NOW, Some(&name));
        let kept = rec.for_whom.expect("the name should be kept");
        assert_eq!(kept.chars().count(), 64);
        assert!(kept.chars().all(|c| c == '𝄞'), "{kept}");
    }
}

/// An issue made `secs` seconds ago (by the `NOW` clock).
fn ago(secs: u64) -> Option<u64> {
    Some(NOW - secs)
}
