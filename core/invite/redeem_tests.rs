//! Tests of the pure parts of the invite join client (`redeem.rs`).
//!
//! No network here: the outcome of a round of polling members (`round_outcome`), the growth of the
//! pause between rounds (`next_pause`) and checking the other side's id (`peer_is`). The end-to-end join over a
//! loop is checked by `core/tests/invite_once_loopback.rs`.

use std::time::Duration;

use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::invite::wire::{RedeemCode, RedeemReply};
use crate::members::network_key::{managed_key, NetworkKey};

mod round_outcome_cases {
    use super::*;

    mod key_among_answers {
        use super::*;

        #[test]
        fn single_answer_gives_the_key() {
            let got = round(&[key(own_key())]);
            assert_eq!(
                key_of(got).map(|k| k.to_bytes()),
                Some(own_key().to_bytes())
            );
        }

        #[test]
        fn after_silence_and_refusals_gives_the_key() {
            let answers = [
                SILENT,
                refused(RedeemCode::Unknown),
                refused(RedeemCode::Busy),
                key(own_key()),
            ];
            let got = key_of(round(&answers));
            assert_eq!(got.map(|k| k.to_bytes()), Some(own_key().to_bytes()));
        }

        #[test]
        fn before_other_answers_gives_the_key() {
            let answers = [key(own_key()), SILENT, refused(RedeemCode::Unknown)];
            let got = key_of(round(&answers));
            assert_eq!(got.map(|k| k.to_bytes()), Some(own_key().to_bytes()));
        }
    }

    mod key_of_foreign_network {
        use super::*;

        #[test]
        fn alone_is_wrong_network() {
            let got = error_of(round(&[key(foreign_key())]));
            assert!(matches!(got, Some(RedeemError::WrongNetwork)), "{got:?}");
        }

        #[test]
        fn after_a_refusal_is_wrong_network() {
            let got = error_of(round(&[refused(RedeemCode::Unknown), key(foreign_key())]));
            assert!(matches!(got, Some(RedeemError::WrongNetwork)), "{got:?}");
        }

        #[test]
        fn is_never_handed_out_as_the_key() {
            let got = key_of(round(&[key(foreign_key())]));
            assert!(
                got.is_none(),
                "a key of a foreign network was accepted as its own: {got:?}"
            );
        }
    }

    mod final_refusal {
        use super::*;

        #[test]
        fn used_is_final() {
            let got = error_of(round(&[refused(RedeemCode::Used)]));
            assert!(matches!(got, Some(RedeemError::Used)), "{got:?}");
        }

        #[test]
        fn expired_is_final() {
            let got = error_of(round(&[refused(RedeemCode::Expired)]));
            assert!(matches!(got, Some(RedeemError::Expired)), "{got:?}");
        }

        #[test]
        fn used_is_final_even_when_others_are_silent_or_unknown() {
            let answers = [
                SILENT,
                refused(RedeemCode::Unknown),
                refused(RedeemCode::Used),
            ];
            let got = error_of(round(&answers));
            assert!(matches!(got, Some(RedeemError::Used)), "{got:?}");
        }

        #[test]
        fn bad_request_is_final_in_legacy_mode() {
            // given: the older frame was sent, so a bad request really is the final word
            let got = error_of(round_in(&[refused(RedeemCode::BadRequest)], Mode::Legacy));
            // then: not a retry
            assert!(
                got.is_some(),
                "BadRequest must not lead to a retry in the legacy mode"
            );
        }

        #[test]
        fn excluded_is_final_in_both_modes() {
            for mode in [Mode::Owner, Mode::Legacy] {
                // given / when: one member says the node is excluded, the others are silent or unknown
                let answers = [
                    SILENT,
                    refused(RedeemCode::Unknown),
                    refused(RedeemCode::Excluded),
                ];
                let got = error_of(round_in(&answers, mode));
                // then
                assert!(
                    matches!(got, Some(RedeemError::Excluded)),
                    "{mode:?}: {got:?}"
                );
            }
        }

        #[test]
        fn used_and_expired_stay_final_in_owner_mode() {
            assert!(matches!(
                error_of(round(&[refused(RedeemCode::Used)])),
                Some(RedeemError::Used)
            ));
            assert!(matches!(
                error_of(round(&[refused(RedeemCode::Expired)])),
                Some(RedeemError::Expired)
            ));
        }
    }

    mod owner_mode_meets_old_members {
        use super::*;

        #[test]
        fn all_bad_request_in_owner_mode_falls_back() {
            // given: every member answered BadRequest to the owner frame: they run an older version
            let got = round(&[
                refused(RedeemCode::BadRequest),
                refused(RedeemCode::BadRequest),
            ]);
            // then
            assert!(is_fallback(&got), "{got:?}");
        }

        #[test]
        fn bad_request_next_to_silence_retries_in_owner_mode() {
            // given: one member is silent: it may be new and know the invite
            let got = round(&[refused(RedeemCode::BadRequest), SILENT]);
            // then: no fallback yet
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn bad_request_next_to_busy_retries() {
            let got = round(&[refused(RedeemCode::BadRequest), refused(RedeemCode::Busy)]);
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn unknown_and_bad_request_count_as_all_unknown() {
            // given: nobody knows the invite or understands the frame, in either order of the answers
            let bad_first = round(&[
                refused(RedeemCode::BadRequest),
                refused(RedeemCode::Unknown),
            ]);
            let unknown_first = round(&[
                refused(RedeemCode::Unknown),
                refused(RedeemCode::BadRequest),
            ]);
            // then: neither a fallback nor a refusal
            assert!(is_all_unknown(&bad_first), "{bad_first:?}");
            assert!(is_all_unknown(&unknown_first), "{unknown_first:?}");
        }

        #[test]
        fn legacy_mode_never_yields_fallback() {
            let cases: [&[Option<RedeemReply>]; 4] = [
                &[refused(RedeemCode::BadRequest)],
                &[
                    refused(RedeemCode::BadRequest),
                    refused(RedeemCode::BadRequest),
                ],
                &[refused(RedeemCode::Unknown), refused(RedeemCode::Busy)],
                &[SILENT],
            ];
            for answers in cases {
                let got = round_in(answers, Mode::Legacy);
                assert!(!is_fallback(&got), "{answers:?} gave {got:?}");
            }
        }
    }

    mod all_who_answered_say_unknown {
        use super::*;

        #[test]
        fn one_answer_is_enough() {
            assert!(is_all_unknown(&round(&[refused(RedeemCode::Unknown)])));
        }

        #[test]
        fn several_answers_give_the_same() {
            let answers = [refused(RedeemCode::Unknown), refused(RedeemCode::Unknown)];
            assert!(is_all_unknown(&round(&answers)));
        }
    }

    mod nothing_conclusive {
        use super::*;

        #[test]
        fn unknown_next_to_silence_retries() {
            // A silent one might know the invite (the issuer is off): "unknown" is not yet the common outcome.
            // "Unknown + silence" means retry, not "all answered Unknown": the silent ones
            // may still know the invite, so the silent answers do count against a final "unknown".
            let got = round(&[refused(RedeemCode::Unknown), SILENT]);
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn unknown_next_to_busy_retries() {
            let got = round(&[refused(RedeemCode::Unknown), refused(RedeemCode::Busy)]);
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn all_silent_retries() {
            let got = round(&[SILENT, SILENT]);
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn empty_round_retries() {
            // Nobody answered, "all who answered said Unknown" is false: "somebody" must answer.
            let got = round(&[]);
            assert!(is_retry(&got), "{got:?}");
        }

        #[test]
        fn retryable_codes_alone_retry() {
            for code in [RedeemCode::NotReady, RedeemCode::Busy, RedeemCode::Internal] {
                let got = round(&[refused(code)]);
                assert!(
                    is_retry(&got),
                    "{code:?} must lead to a retry, but gave {got:?}"
                );
            }
        }
    }
}

mod next_pause_cases {
    use super::*;

    #[test]
    fn doubles_from_five_to_the_cap_and_stays_there() {
        let mut seen = vec![secs(5)];
        for _ in 0..7 {
            let last = *seen.last().unwrap();
            seen.push(next_pause(last, secs(60)));
        }
        let want: Vec<Duration> = [5, 10, 20, 40, 60, 60, 60, 60].map(secs).to_vec();
        assert_eq!(seen, want);
    }

    #[test]
    fn double_above_the_cap_is_cut_to_the_cap() {
        assert_eq!(next_pause(secs(45), secs(60)), secs(60));
    }

    #[test]
    fn at_the_cap_stays_at_the_cap() {
        assert_eq!(next_pause(secs(60), secs(60)), secs(60));
    }

    #[test]
    fn works_on_sub_second_steps() {
        let ms = Duration::from_millis;
        assert_eq!(next_pause(ms(100), ms(400)), ms(200));
        assert_eq!(next_pause(ms(200), ms(400)), ms(400));
        assert_eq!(next_pause(ms(400), ms(400)), ms(400));
    }
}

mod peer_is_cases {
    use super::*;

    #[test]
    fn same_id_is_accepted() {
        assert!(peer_is(id(1), id(1)));
    }

    #[test]
    fn other_id_is_rejected() {
        assert!(
            !peer_is(id(1), id(2)),
            "a peer with a foreign id was accepted"
        );
    }
}

// `Outcome` is read as `Key(NetworkKey) | Refused(RedeemError) | AllUnknown | Retry`;
// only the helper block below depends on these names.

fn round(answers: &[Option<RedeemReply>]) -> Outcome {
    round_in(answers, Mode::Owner)
}

fn round_in(answers: &[Option<RedeemReply>], mode: Mode) -> Outcome {
    round_outcome(answers, &head(), mode)
}

fn is_fallback(o: &Outcome) -> bool {
    matches!(o, Outcome::Legacy)
}

fn key_of(o: Outcome) -> Option<NetworkKey> {
    match o {
        Outcome::Key(k) => Some(k),
        _ => None,
    }
}

fn error_of(o: Outcome) -> Option<RedeemError> {
    match o {
        Outcome::Refused(e) => Some(e),
        _ => None,
    }
}

fn is_all_unknown(o: &Outcome) -> bool {
    matches!(o, Outcome::AllUnknown)
}

fn is_retry(o: &Outcome) -> bool {
    matches!(o, Outcome::Retry)
}

/// A member that did not answer (the connection did not happen, the frame did not arrive).
const SILENT: Option<RedeemReply> = None;

fn refused(code: RedeemCode) -> Option<RedeemReply> {
    Some(RedeemReply::Refused(code))
}

fn key(k: NetworkKey) -> Option<RedeemReply> {
    Some(RedeemReply::Key(k))
}

/// The head of an invite into the network made from `own_key()` and the creator `id(7)`.
fn head() -> InviteHead {
    InviteHead {
        secret: crate::invite::InviteSecret::from_bytes([1; 32]),
        network_id: managed_key(&own_key().to_bytes(), &id(7))
            .subkeys()
            .network_id,
        label: None,
        made_unix: 0,
        root: id(7),
    }
}

/// The base of the network key: what a member answers with.
fn own_key() -> NetworkKey {
    NetworkKey::from_bytes([1; 32])
}

/// The key of another network; the `network_id` is different, otherwise the "foreign network" test would check nothing.
fn foreign_key() -> NetworkKey {
    let foreign = NetworkKey::from_bytes([2; 32]);
    assert_ne!(foreign.subkeys().network_id, own_key().subkeys().network_id);
    foreign
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn id(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

// Closing the transport on the `redeem_with` exit — neighbor files (this file is already at the line ceiling).
#[path = "redeem_close_tests.rs"]
mod closes_transport;
#[path = "redeem_round_tests.rs"]
mod managed_round;
#[path = "redeem_mem_tests.rs"]
mod mem_endpoint;
