//! Tests of `DialRefusal::of`: who refused a handshake, read from the error chain. Our own refusal wins over
//! the peer's; a plain error is no refusal.

use anyhow::anyhow;

use crate::members::access::view::Deny;
use crate::members::handshake::{RefusedByPeer, RefusedByUs};

use super::*;

mod of {
    use super::*;

    #[test]
    fn keeps_every_local_reason() {
        for deny in [
            Deny::Left,
            Deny::Excluded,
            Deny::VotedByMe,
            Deny::Blocked,
            Deny::Forgotten,
            Deny::RemovedByMe,
            Deny::Unlisted,
        ] {
            // given: our own list refused the peer with this reason
            let e = anyhow::Error::from(RefusedByUs(deny));

            // when: the refusal is read from the error
            let refusal = DialRefusal::of(&e);

            // then: it is our refusal with the same reason
            assert_eq!(refusal, DialRefusal::ByUs(deny), "reason {deny:?}");
        }
    }

    #[test]
    fn keeps_the_peers_reason() {
        for deny in [Deny::Unlisted, Deny::Excluded] {
            // given: the other side closed the connection with a refusal
            let e = anyhow!("closed").context(RefusedByPeer(deny));

            // when: the refusal is read from the error
            let refusal = DialRefusal::of(&e);

            // then: it is the peer's refusal with the same reason
            assert_eq!(refusal, DialRefusal::ByPeer(deny), "reason {deny:?}");
        }
    }

    #[test]
    fn prefers_our_refusal_over_the_peers() {
        // given: an error that carries both our refusal and the peer's
        let e =
            anyhow::Error::from(RefusedByUs(Deny::Blocked)).context(RefusedByPeer(Deny::Excluded));

        // when: the refusal is read from the error
        let refusal = DialRefusal::of(&e);

        // then: our own refusal wins
        assert_eq!(refusal, DialRefusal::ByUs(Deny::Blocked));
    }

    #[test]
    fn plain_error_is_not_a_refusal() {
        // given: a timeout, no refusal anywhere in the chain
        let e = anyhow!("timeout");

        // when: the refusal is read from the error
        let refusal = DialRefusal::of(&e);

        // then: nobody refused
        assert_eq!(refusal, DialRefusal::NotRefused);
    }
}
