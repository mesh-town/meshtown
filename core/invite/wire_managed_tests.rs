//! Tests of the invite wire codes: the code numbers, the retry flag, the mapping from issue verdicts
//! and the refusal for a key not issued by an administrator (`wire.rs`).

use super::*;

mod code_numbers {
    use super::*;

    #[test]
    fn values_are_fixed() {
        let want = [
            (RedeemCode::Ok, 0u8),
            (RedeemCode::NotReady, 1),
            (RedeemCode::Busy, 2),
            (RedeemCode::Used, 3),
            (RedeemCode::Expired, 4),
            (RedeemCode::Unknown, 5),
            (RedeemCode::BadRequest, 6),
            (RedeemCode::Internal, 7),
            (RedeemCode::Excluded, 8),
        ];
        for (code, number) in want {
            assert_eq!(code as u8, number, "{code:?}");
        }
    }

    #[test]
    fn excluded_is_code_8_and_not_retryable() {
        assert_eq!(RedeemCode::Excluded as u8, 8);
        assert_eq!(RedeemCode::from_u8(8), Some(RedeemCode::Excluded));
        assert!(
            !RedeemCode::Excluded.is_retryable(),
            "an excluded key stays excluded: no point in retrying"
        );
    }

    #[test]
    fn code_10_is_unknown() {
        assert_eq!(RedeemCode::from_u8(10), None);
    }

    #[test]
    fn from_u8_inverts_the_numbers() {
        for (code, number) in all_codes() {
            assert_eq!(RedeemCode::from_u8(number), Some(code), "number {number}");
        }
    }

    #[test]
    fn from_u8_rejects_numbers_past_the_last_code() {
        for number in [10u8, 11, 100, 255] {
            assert_eq!(RedeemCode::from_u8(number), None, "number {number}");
        }
    }
}

mod retry_flag {
    use super::*;

    #[test]
    fn true_exactly_for_not_ready_busy_and_internal() {
        for (code, number) in all_codes() {
            let must_retry = matches!(number, 1 | 2 | 7);
            assert_eq!(
                code.is_retryable(),
                must_retry,
                "{code:?} (number {number})"
            );
        }
    }
}

mod code_from_verdict {
    use super::*;

    #[test]
    fn every_verdict_maps_to_its_own_code() {
        for verdict in all_verdicts() {
            let want = match verdict {
                IssueVerdict::Grant => RedeemCode::Ok,
                IssueVerdict::Used => RedeemCode::Used,
                IssueVerdict::Expired => RedeemCode::Expired,
                IssueVerdict::Unknown => RedeemCode::Unknown,
                IssueVerdict::NotReady => RedeemCode::NotReady,
                IssueVerdict::NotAdmin => RedeemCode::NotFromAdmin,
            };
            assert_eq!(code_from(verdict), want);
        }
    }

    #[test]
    fn not_ready_is_the_only_retried_verdict() {
        // "Document not ready yet" — retry; redeemed, expired, unknown — final.
        for verdict in all_verdicts() {
            let retried = matches!(verdict, IssueVerdict::NotReady);
            assert_eq!(code_from(verdict).is_retryable(), retried);
        }
    }
}

mod not_from_admin {
    use super::*;

    #[test]
    fn is_code_9_and_round_trips() {
        assert_eq!(RedeemCode::NotFromAdmin as u8, 9);
        assert_eq!(RedeemCode::from_u8(9), Some(RedeemCode::NotFromAdmin));
    }

    #[test]
    fn code_10_is_unknown() {
        assert_eq!(RedeemCode::from_u8(10), None);
    }

    #[test]
    fn refusal_frame_survives_a_round_trip() {
        let frame = encode_invite_reply(RedeemCode::NotFromAdmin, None);
        assert_eq!(frame[1], 9);
        assert_eq!(
            refusal_in(parse_invite_reply(&frame)),
            Some(RedeemCode::NotFromAdmin)
        );
    }

    #[test]
    fn is_final_not_retryable() {
        // assumed: a refusal for a key not issued by an administrator is final, like `Excluded`.
        assert!(!RedeemCode::NotFromAdmin.is_retryable());
    }
}
