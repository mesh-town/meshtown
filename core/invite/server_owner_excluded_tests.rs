//! Tests of the invite server's owner path when the server itself is excluded from the membership list:
//! it hands out no key to anybody, including the computer it runs on, and the refusal is retryable.
//! Also the refusal code the owner path gives for each verdict of the membership view.
//! The fixture and the helpers are in `server_owner_support.rs`.

use super::support::*;
use super::*;
use crate::members::access::tally::{tally, Exclusion, How, Ledger as Votes};
use crate::transport::net::facade::test_support::{pid, secret};

mod owner_code_of_the_verdict {
    use super::*;

    #[test]
    fn return_cases_go_on() {
        // given / when / then: no refusal for a member or an unknown key
        for case in [Ok(()), Err(Deny::Unlisted)] {
            assert_eq!(owner_code(case), None, "{case:?} must let the owner go on");
        }
    }

    #[test]
    fn excluded_stays_final() {
        assert_eq!(owner_code(Err(Deny::Excluded)), Some(RedeemCode::Excluded));
    }

    #[test]
    fn own_block_and_leaving_stay_retryable() {
        for case in [Deny::Blocked, Deny::Left, Deny::Refused] {
            assert_eq!(
                owner_code(Err(case)),
                Some(RedeemCode::NotReady),
                "{case:?} must be a retryable refusal"
            );
        }
    }
}

/// A ledger of a network where the server is a member that the others excluded.
fn excluded_server_ledger(members: &[PeerId]) -> Votes {
    let mut ledger = tally(&input_of(members));
    ledger.excluded.insert(
        me(),
        Exclusion {
            how: How::Votes,
            at_micros: 10,
            voters: vec![pid(3)],
        },
    );
    ledger
}

#[tokio::test]
async fn an_excluded_server_answers_not_ready() {
    // given: the server is excluded and a foreign newcomer presents the invite
    let node = setup_node(access_with(excluded_server_ledger(&[
        me(),
        pid(3),
        owner_id(),
    ])))
    .await;
    // when
    let reply = node.when_owner_asks(&secret(7)).await;
    // then: a retryable refusal, nothing is written, the invite is still good
    assert_refused(reply, RedeemCode::NotReady);
    node.then_invite_unspent().await;
}

#[tokio::test]
async fn an_excluded_server_answers_not_ready_to_its_own_key() {
    // given: the server is excluded and the owner is the server's own key
    let node = setup_node(access_with(excluded_server_ledger(&[me(), pid(3)]))).await;
    // when: the computer the server runs on presents the invite under its own key
    let reply = node.when_owner_asks(&secret(2)).await;
    // then: the server does not hand out the key to itself either
    assert_refused(reply, RedeemCode::NotReady);
    node.then_invite_unspent().await;
}
