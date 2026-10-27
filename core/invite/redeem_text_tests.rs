//! Tests of the wording of the refusals shown to the person who redeems an invite.

use super::RedeemError;

#[test]
fn excluded_text_is_for_a_computer_that_cannot_rejoin() {
    assert_eq!(
        RedeemError::Excluded.to_string(),
        "This computer can't rejoin this network. The invite wasn't used up — someone else can join with it."
    );
}

#[test]
fn not_from_admin_text_names_the_current_admins() {
    assert_eq!(
        RedeemError::NotFromAdmin.to_string(),
        "This invite can't be used: whoever made it is not a network admin now. Ask a network admin for a new one."
    );
}

#[test]
fn used_text_says_computer_and_whoever_sent_it() {
    let text = RedeemError::Used.to_string();
    assert!(
        !text.contains("network member") && !text.contains("node"),
        "{text}"
    );
    assert!(text.contains("whoever sent it"), "{text}");
}

#[test]
fn expired_text_says_no_member_let_in() {
    let text = RedeemError::Expired.to_string();
    assert!(!text.contains("network member"), "{text}");
    assert!(text.contains("no member let this computer in"), "{text}");
    assert_eq!(text, crate::invite::head::TEXT_EXPIRED);
}

#[test]
fn unknown_text_asks_whoever_sent_it() {
    assert!(RedeemError::Unknown.to_string().contains("whoever sent it"));
}
