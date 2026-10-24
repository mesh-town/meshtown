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
        "This invite can't be used: whoever made it is not an admin of this network now. Ask an admin of the network for a new one."
    );
}
