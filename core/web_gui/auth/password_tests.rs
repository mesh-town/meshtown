//! Tests for `password.rs`: hash verdicts, normalization, password format.

use super::*;

fn n(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).unwrap()
}

/// 1000 iterations is fewer than in production, so the right password gives `MatchNeedsRehash`; what matters here is "accepted".
fn accepted(v: PasswordVerdict) -> bool {
    matches!(
        v,
        PasswordVerdict::Match | PasswordVerdict::MatchNeedsRehash
    )
}

mod fast {
    use super::*;

    fn rec() -> PasswordRecord {
        hash_with("correct horse", n(1000)).unwrap()
    }

    #[test]
    fn right_password_is_ok() {
        let r = rec();
        assert!(accepted(verify(&r, "correct horse")));
    }

    #[test]
    fn wrong_password_is_wrong() {
        assert_eq!(verify(&rec(), "correct horsf"), PasswordVerdict::Mismatch);
    }

    #[test]
    fn two_hashes_have_different_salt_and_hash() {
        let a = rec();
        let b = rec();
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn nfd_input_matches_nfc_record() {
        let r = hash_with("caf\u{e9}-pass", n(1000)).unwrap();
        assert!(accepted(verify(&r, "cafe\u{301}-pass")));
    }

    #[test]
    fn over_256_chars_is_wrong() {
        let long = "a".repeat(257);
        assert_eq!(verify(&rec(), &long), PasswordVerdict::Mismatch);
    }
}

mod old_iterations {
    use super::*;

    fn rec() -> PasswordRecord {
        hash_with("correct horse", n(PBKDF2_ITERATIONS - 1)).unwrap()
    }

    #[test]
    fn right_password_is_ok_rehash() {
        assert_eq!(
            verify(&rec(), "correct horse"),
            PasswordVerdict::MatchNeedsRehash
        );
    }

    #[test]
    fn wrong_password_is_wrong() {
        assert_eq!(verify(&rec(), "nope nope nope"), PasswordVerdict::Mismatch);
    }
}

mod real {
    use super::*;

    #[test]
    fn hash_uses_600k_and_verifies() {
        let r = hash("correct horse").unwrap();
        assert_eq!(r.iterations.get(), 600_000);
        assert_eq!(verify(&r, "correct horse"), PasswordVerdict::Match);
    }
}

mod format {
    use super::*;

    #[test]
    fn seven_chars_rejected() {
        assert!(check_format("1234567").is_err());
    }

    #[test]
    fn eight_chars_accepted() {
        assert!(check_format("12345678").is_ok());
    }

    #[test]
    fn chars_256_accepted_257_rejected() {
        assert!(check_format(&"a".repeat(256)).is_ok());
        assert!(check_format(&"a".repeat(257)).is_err());
    }

    #[test]
    fn counts_chars_not_bytes() {
        assert!(check_format("éøüéøü12").is_ok());
        assert!(check_format("éøüéøü1").is_err());
    }
}
