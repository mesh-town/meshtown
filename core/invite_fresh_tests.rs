//! Tests of `invite.rs`: check fresh cases.

use super::*;

#[test]
fn made_just_now_is_accepted() {
    assert!(check_fresh(&head_made_at(NOW), NOW).is_ok());
}

#[test]
fn made_twenty_three_hours_ago_is_accepted() {
    assert!(check_fresh(&head_made_at(NOW), NOW + 23 * HOUR).is_ok());
}

#[test]
fn one_second_before_the_ttl_is_accepted() {
    assert!(check_fresh(&head_made_at(NOW), NOW + INVITE_TTL_SECS - 1).is_ok());
}

#[test]
fn exactly_at_the_ttl_is_refused() {
    // The boundary is read as in `decide`: fresh strictly when `now < made_unix + TTL`.
    let result = check_fresh(&head_made_at(NOW), NOW + INVITE_TTL_SECS);
    assert!(
        result.is_err(),
        "at the TTL boundary the invite must be expired"
    );
}

#[test]
fn made_twenty_five_hours_ago_is_refused_with_the_expired_text() {
    let err = check_fresh(&head_made_at(NOW), NOW + 25 * HOUR).unwrap_err();
    let text = format!("{err:#}");
    assert_eq!(text, crate::invite::head::TEXT_EXPIRED);
    assert!(
        !text.contains("old format"),
        "T1 text instead of T2: {text}"
    );
}

#[test]
fn made_ahead_of_our_clock_is_accepted() {
    // The issuer's clock ran ahead: an invite "from the future" is not expired.
    assert!(check_fresh(&head_made_at(NOW + 500), NOW).is_ok());
}

#[test]
fn ttl_is_one_day() {
    assert_eq!(INVITE_TTL_SECS, 86_400);
}
