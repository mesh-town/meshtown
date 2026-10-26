//! Tests of the round outcome (`round_outcome`) for a Managed network: the refusal "not from an admin" and the
//! check of the answered base against the invite's network id after it is combined with the creator.
//!
//! assumed: declared next to the other round tests (`redeem_tests.rs`, bottom, `#[path]`), and
//! `round_outcome(answers: &[Option<RedeemReply>], head: &InviteHead, mode: Mode) -> Outcome`, where the key
//! of an answer is compared as `head.reply_key(answer).subkeys().network_id == head.network_id` and
//! `Outcome::Key` carries the raw answer. `RedeemError::NotFromAdmin` is the final refusal of code 9.

use crate::invite::redeem::RedeemError;
use crate::invite::redeem_round::{round_outcome, Mode, Outcome};
use crate::invite::wire::{RedeemCode, RedeemReply};
use crate::invite::{InviteHead, InviteSecret};
use crate::members::network_key::{managed_key, NetworkKey};
use crate::transport::net::NodeSecret;

const BASE: [u8; 32] = [1; 32];
const OTHER_BASE: [u8; 32] = [2; 32];

mod not_from_admin {
    use super::*;

    #[test]
    fn is_final_in_owner_mode() {
        // given: a member answers that the invite was not issued by an admin
        let head = managed_head();
        // when
        let got = round(&head, &[refused(RedeemCode::NotFromAdmin)], Mode::Owner);
        // then: the round is decisive, there is nothing to retry
        assert!(got.decisive(), "{got:?}");
        assert!(
            matches!(got, Outcome::Refused(RedeemError::NotFromAdmin)),
            "{got:?}"
        );
    }

    #[test]
    fn is_final_in_legacy_mode() {
        // given
        let head = managed_head();
        // when
        let got = round(&head, &[refused(RedeemCode::NotFromAdmin)], Mode::Legacy);
        // then
        assert!(
            matches!(got, Outcome::Refused(RedeemError::NotFromAdmin)),
            "{got:?}"
        );
    }

    #[test]
    fn stays_final_next_to_silence_and_busy_members() {
        // given: others are silent, busy or do not know the invite
        let head = managed_head();
        let answers = [
            None,
            refused(RedeemCode::Busy),
            refused(RedeemCode::Unknown),
            refused(RedeemCode::NotFromAdmin),
        ];
        // when
        let got = round(&head, &answers, Mode::Owner);
        // then
        assert!(
            matches!(got, Outcome::Refused(RedeemError::NotFromAdmin)),
            "{got:?}"
        );
    }
}

mod answered_base {
    use super::*;

    #[test]
    fn of_the_network_gives_the_raw_base() {
        // given: the invite names the creator, the member answers with the base
        let head = managed_head();
        // when
        let got = round(&head, &[key(BASE)], Mode::Owner);
        // then: the outcome carries the base itself, the caller derives the key from it
        let Outcome::Key(kept) = got else {
            panic!("expected the key, got {got:?}");
        };
        assert_eq!(kept.to_bytes(), BASE, "the raw answer must be kept");
    }

    #[test]
    fn swapped_for_another_is_wrong_network() {
        // given: the serving node substituted the base
        let head = managed_head();
        // when
        let got = round(&head, &[key(OTHER_BASE)], Mode::Owner);
        // then: with the creator it derives another network id
        assert!(
            matches!(got, Outcome::Refused(RedeemError::WrongNetwork)),
            "{got:?}"
        );
    }

    #[test]
    fn already_derived_key_is_wrong_network() {
        // given: a server answers with the derived key instead of the base
        let head = managed_head();
        let derived = managed_key(&BASE, &creator()).to_bytes();
        // when
        let got = round(&head, &[key(derived)], Mode::Owner);
        // then
        assert!(
            matches!(got, Outcome::Refused(RedeemError::WrongNetwork)),
            "{got:?}"
        );
    }
}

/// `round_outcome` over a head; the one place that knows how the head is passed.
fn round(head: &InviteHead, answers: &[Option<RedeemReply>], mode: Mode) -> Outcome {
    round_outcome(answers, head, mode)
}

/// The head of an invite into a Managed network made from `BASE` and `creator()`.
fn managed_head() -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([5; 32]),
        network_id: managed_key(&BASE, &creator()).subkeys().network_id,
        label: None,
        made_unix: 1_790_000_000,
        root: creator(),
    }
}

fn creator() -> crate::transport::net::PeerId {
    NodeSecret::from_bytes(&[7; 32]).peer_id()
}

fn refused(code: RedeemCode) -> Option<RedeemReply> {
    Some(RedeemReply::Refused(code))
}

fn key(bytes: [u8; 32]) -> Option<RedeemReply> {
    Some(RedeemReply::Key(NetworkKey::from_bytes(bytes)))
}
