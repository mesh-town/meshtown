//! Tests of `Country`: parsing valid and invalid codes.

use super::Country;

#[test]
fn parses_valid_codes() {
    assert_eq!(Country::parse("RU").unwrap().as_str(), "RU");
    assert_eq!(Country::parse("ru").unwrap().as_str(), "RU");
    assert_eq!(Country::from_bytes(b"DE").unwrap().as_str(), "DE");
}

#[test]
fn rejects_invalid_codes() {
    for bad in ["", "R", "RUS", "R1", "XX", "Éé"] {
        assert!(Country::parse(bad).is_none(), "{bad:?} must be invalid");
    }
    assert!(Country::from_bytes(&[0xFF, 0xFE]).is_none());
}
