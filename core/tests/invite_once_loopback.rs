//! A one-time invite on loopback: nodes without DHT and without relay, the network key is handed out by a member via
//! the `meshtown/invite/1` protocol after checking the one-time secret.
//!
//! What it does: checks joining by a fresh secret, a refusal on repeat, a single key under
//! simultaneous presentations, a switch to another admitter, expiry and cleanup, a refusal for a foreign
//! peer id and for a foreign network's key.
//! What it doesn't do: it doesn't build the `mesh-invite2-` string (the head and addresses are assembled in the test), doesn't
//! check DHT, relay, the console and the page; server overload (`Busy`) is in `wire_tests`.
//! Links: `meshtown_core::invite::{redeem, store}`, `node::start_node`, `NodeConfig::initial_peers`.

use std::time::Duration;

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::invite::store::IssueVerdict;
use meshtown_core::invite::INVITE_TTL_SECS;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node;
use meshtown_core::transport::net_mainline::unix_now;

#[path = "common/invite_nodes.rs"]
mod common;
use common::{
    addr_of, fresh_head, loopback_addr, member_config, quick_opts, record_issue, redeem_via, setup,
    setup_serving, short_opts, start, start_second_admin, stranger_id, wait_until, wait_verdict,
    ENTRY_LIMIT_SECS, HOUR, OWNER_SEED,
};

/// Rounds in the simultaneous-repeat test: a race without the server lock doesn't show up in every round.
const ROUNDS: u32 = 12;
/// The permanent keys of the second admin and of the newcomer (the one the redemption proved).
const SECOND_SEED: u8 = 41;
const NEWCOMER_SEED: u8 = OWNER_SEED;

mod entry {
    use super::*;

    mod fresh_secret_at_the_issuer {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn gives_the_network_key_of_the_issuer() {
            // given
            let rig = setup().await;
            let head = rig.issue(unix_now()).await;
            // when
            let got = rig.redeem(&head).await;
            // then
            let got = got.expect("a fresh secret must yield the network key");
            assert_eq!(got.to_bytes(), rig.key.to_bytes());
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn newcomer_with_that_key_becomes_a_member() {
            // given
            let rig = setup().await;
            let head = rig.issue(unix_now()).await;
            // when
            let got = rig
                .redeem(&head)
                .await
                .expect("a fresh secret must yield the network key");
            let mut cfg = member_config(&got, NEWCOMER_SEED);
            cfg.initial_peers = vec![rig.addr.clone()];
            let newcomer = node::start_node(cfg).await.unwrap();
            // then
            let joined = wait_until(Duration::from_secs(20), || {
                rig.a.members.is_member(newcomer.id) && newcomer.members.is_member(rig.a.id)
            })
            .await;
            assert!(
                joined,
                "the nodes did not become members of each other within 20 s after joining"
            );
        }
    }
}

mod repeat_of_spent_secret {
    use super::*;

    mod same_issuer {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_refused_as_used() {
            // given
            let rig = setup().await;
            let head = rig.issue(unix_now()).await;
            rig.redeem(&head)
                .await
                .expect("the first join must succeed");
            // when
            let second = rig.redeem(&head).await;
            // then
            assert!(
                matches!(second, Err(RedeemError::Used)),
                "the repeat gave {second:?}"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn issuer_writes_the_used_stamp_into_the_document() {
            // given
            let rig = setup().await;
            let head = rig.issue(unix_now()).await;
            // when
            rig.redeem(&head)
                .await
                .expect("the first join must succeed");
            // then
            assert_eq!(rig.verdict(&head).await, IssueVerdict::Used);
        }
    }

    mod two_at_once {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn gives_exactly_one_key_and_one_used() {
            // given
            let rig = setup().await;
            for round in 0..ROUNDS {
                let head = rig.issue(unix_now()).await;
                // when
                let (first, second) = tokio::join!(rig.redeem(&head), rig.redeem(&head));
                // then
                let keys = [&first, &second].iter().filter(|r| r.is_ok()).count();
                let used = [&first, &second]
                    .iter()
                    .filter(|r| matches!(r, Err(RedeemError::Used)))
                    .count();
                assert_eq!(
                    (keys, used),
                    (1, 1),
                    "round {round}: {first:?} and {second:?}"
                );
            }
        }
    }

    mod other_issuer {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_refused_as_used_once_the_stamp_has_arrived() {
            // given: A issued, B saw the issue, N1 joined through A, B saw the label
            let rig = setup().await;
            let b = start_second_admin(&rig.a, &rig.key, SECOND_SEED).await;
            let head = rig.issue(unix_now()).await;
            assert!(
                wait_verdict(&b, &head, IssueVerdict::Grant, 15).await,
                "A's issue did not reach B within 15 s"
            );
            rig.redeem(&head).await.expect("N1 must join through A");
            assert!(
                wait_verdict(&b, &head, IssueVerdict::Used, 15).await,
                "the redemption label from A did not reach B within 15 s"
            );
            // when: N2 comes to the second admitter, the only address in the head is B
            let second =
                redeem_via(&head, &[addr_of(&b)], quick_opts(&head, ENTRY_LIMIT_SECS)).await;
            // then
            assert!(
                matches!(second, Err(RedeemError::Used)),
                "N2 at B got {second:?}"
            );
        }
    }
}

mod expired_issue {
    use super::*;

    mod issued_twenty_five_hours_ago {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_refused_as_expired() {
            // given: the head is fresh (the client's deadline has not passed), the issue record has expired
            let rig = setup().await;
            let head = rig.issue(unix_now() - 25 * HOUR).await;
            // when
            let got = rig.redeem(&head).await;
            // then
            assert!(
                matches!(got, Err(RedeemError::Expired)),
                "expiry gave {got:?}"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn sweep_removes_the_record_and_verdict_turns_unknown() {
            // given
            let rig = setup().await;
            let head = rig.issue(unix_now() - 25 * HOUR).await;
            assert_eq!(
                rig.verdict(&head).await,
                IssueVerdict::Expired,
                "before cleanup the issue is expired"
            );
            // when
            let later = unix_now() + INVITE_TTL_SECS + 1;
            let removed = rig.ledger().sweep(later).await.unwrap();
            // then
            assert!(removed >= 1, "the cleanup removed no records");
            let hash = head.secret.hash_hex();
            assert_eq!(
                rig.ledger().verdict(&hash, later, None).await,
                IssueVerdict::Unknown
            );
        }
    }
}

mod wrong_peer_id {
    use super::*;

    mod address_of_issuer_under_id_of_stranger {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn ends_as_expired_at_the_deadline() {
            // given: an address with the id of node C, which is not in the `mem` network — you can't reach A under a foreign id
            let rig = setup_serving().await;
            let head = rig.issue(unix_now()).await;
            let wrong = loopback_addr(stranger_id(), &rig.a);
            // when
            let got = redeem_via(&head, &[wrong], short_opts(&head, 3)).await;
            // then
            assert!(
                matches!(got, Err(RedeemError::Expired)),
                "under a foreign id it gave {got:?}"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn secret_stays_unspent_at_the_issuer() {
            // given
            let rig = setup_serving().await;
            let head = rig.issue(unix_now()).await;
            let wrong = loopback_addr(stranger_id(), &rig.a);
            // when
            let _ = redeem_via(&head, &[wrong], short_opts(&head, 3)).await;
            // then
            assert_eq!(
                rig.verdict(&head).await,
                IssueVerdict::Grant,
                "the secret went to the wrong node"
            );
        }
    }
}

mod impostor_of_other_network {
    use super::*;

    mod node_from_the_string_answers_with_foreign_key {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_refused_as_wrong_network() {
            // given: the head has network R, and node D from the string is of another network and honestly hands out its own key
            let real = NetworkKey::generate().unwrap();
            let other = NetworkKey::generate().unwrap();
            assert_ne!(real.subkeys().network_id, other.subkeys().network_id);
            let d = start(&other).await;
            let head = fresh_head(&real, unix_now());
            record_issue(&d, &head, unix_now()).await;
            // when
            let got = redeem_via(&head, &[addr_of(&d)], quick_opts(&head, ENTRY_LIMIT_SECS)).await;
            // then
            assert!(
                matches!(got, Err(RedeemError::WrongNetwork)),
                "the impostor gave {got:?}"
            );
        }
    }
}
