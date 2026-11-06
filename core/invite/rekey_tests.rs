//! Tests of `redeem_or_rekey`: a fresh node key is tried only after the issuer refused the old one as
//! excluded, and only once.

use super::*;
use crate::transport::net::facade::test_support::secret;
use crate::transport::net::PeerId;
use std::collections::VecDeque;
use std::sync::Mutex;

mod accepted_first {
    use super::*;

    #[tokio::test]
    async fn accepted_first_keeps_the_key() {
        // given
        let run = setup(vec![Ok(net_key())]);
        // when
        let got = run.when_redeemed().await.expect("the join succeeds");
        // then
        assert_eq!(got.key.to_bytes(), net_key().to_bytes());
        assert!(got.new_owner.is_none(), "the node key must stay as it was");
        run.then_attempts_by(&[me().peer_id()]);
        run.then_fresh_was_not_asked();
    }
}

mod excluded_first {
    use super::*;

    #[tokio::test]
    async fn excluded_then_a_fresh_key_is_accepted() {
        // given
        let run = setup(vec![Err(RedeemError::Excluded), Ok(net_key())]);
        // when
        let got = run
            .when_redeemed()
            .await
            .expect("the second attempt succeeds");
        // then
        let new = got.new_owner.expect("the fresh key is returned");
        assert_eq!(got.key.to_bytes(), net_key().to_bytes());
        assert_ne!(new.peer_id(), me().peer_id(), "the key must be a new one");
        assert_eq!(new.peer_id(), secret(99).peer_id());
        run.then_attempts_by(&[me().peer_id(), new.peer_id()]);
    }

    #[tokio::test]
    async fn second_refusal_is_final() {
        for second in [
            RedeemError::Excluded,
            RedeemError::Expired,
            RedeemError::Used,
        ] {
            // given
            let run = setup(vec![Err(RedeemError::Excluded), Err(second.clone())]);
            // when
            let got = run.when_redeemed().await;
            // then: the refusal of the second attempt is the outcome, no third attempt, no key returned
            assert_eq!(got.err(), Some(second.clone()), "second refusal {second:?}");
            assert_eq!(run.attempts(), 2, "second refusal {second:?}");
        }
    }

    #[tokio::test]
    async fn no_fresh_key_is_a_bind_error() {
        // given
        let run = setup_without_fresh(vec![Err(RedeemError::Excluded), Ok(net_key())]);
        // when
        let got = run.when_redeemed().await;
        // then
        assert!(matches!(got, Err(RedeemError::Bind(_))), "{:?}", got.err());
        assert_eq!(run.attempts(), 1, "no second attempt without a fresh key");
    }
}

mod other_refusals {
    use super::*;

    #[tokio::test]
    async fn other_refusals_do_not_rekey() {
        for refusal in [
            RedeemError::Used,
            RedeemError::Expired,
            RedeemError::Unknown,
            RedeemError::WrongNetwork,
            RedeemError::NotFromAdmin,
            RedeemError::Bind("no socket".into()),
        ] {
            // given
            let run = setup(vec![Err(refusal.clone()), Ok(net_key())]);
            // when
            let got = run.when_redeemed().await;
            // then
            assert_eq!(got.err(), Some(refusal.clone()), "refusal {refusal:?}");
            assert_eq!(run.attempts(), 1, "refusal {refusal:?}");
            run.then_fresh_was_not_asked();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// setup and helpers
// ---------------------------------------------------------------------------------------------

/// A scripted `attempt`: returns results in order and remembers who tried.
struct Run {
    results: Mutex<VecDeque<Result<NetworkKey, RedeemError>>>,
    tried_by: Mutex<Vec<PeerId>>,
    fresh_asked: Mutex<bool>,
    fresh_works: bool,
}

fn setup(results: Vec<Result<NetworkKey, RedeemError>>) -> Run {
    Run::new(results, true)
}

fn setup_without_fresh(results: Vec<Result<NetworkKey, RedeemError>>) -> Run {
    Run::new(results, false)
}

impl Run {
    fn new(results: Vec<Result<NetworkKey, RedeemError>>, fresh_works: bool) -> Run {
        Run {
            results: Mutex::new(results.into()),
            tried_by: Mutex::default(),
            fresh_asked: Mutex::new(false),
            fresh_works,
        }
    }

    async fn when_redeemed(&self) -> Result<Redeemed, RedeemError> {
        let fresh = || {
            *self.fresh_asked.lock().unwrap() = true;
            match self.fresh_works {
                true => Ok(secret(99)),
                false => Err(RedeemError::Bind("no randomness".into())),
            }
        };
        let attempt = |owner: NodeSecret| {
            self.tried_by.lock().unwrap().push(owner.peer_id());
            let next = self.results.lock().unwrap().pop_front();
            std::future::ready(next.expect("an attempt that the script did not expect"))
        };
        redeem_or_rekey(&me(), fresh, attempt).await
    }

    fn attempts(&self) -> usize {
        self.tried_by.lock().unwrap().len()
    }

    fn then_attempts_by(&self, want: &[PeerId]) {
        let got = self.tried_by.lock().unwrap().clone();
        assert_eq!(got, want, "who tried the invite, in order");
    }

    fn then_fresh_was_not_asked(&self) {
        assert!(
            !*self.fresh_asked.lock().unwrap(),
            "a fresh key was generated although the first answer was not `excluded`"
        );
    }
}

fn me() -> NodeSecret {
    secret(5)
}

fn net_key() -> NetworkKey {
    NetworkKey::from_bytes([3; 32])
}
