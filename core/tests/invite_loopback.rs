//! Joining the network by a real invite string without DHT: the string is built by the admitter node's path
//! (`invite::make` → `Issued::commit`, like the console and the page), the newcomer gets the network key from
//! a member through `redeem` and joins the network.
//!
//! What it does: checks that `make` doesn't touch the document while `commit` writes the issue; that the string holds
//! the network head and the admitter's address but no key; that the newcomer gets the key from the string, becomes
//! a member and that the same string doesn't pass a second time.
//! What it doesn't do: it doesn't check DHT, relay, the console and the page; the protocol scenarios (repeat,
//! expiry, a second admitter, a foreign id, a foreign network) are in `invite_once_loopback.rs`. The address in
//! the string is the admitter's id without hints: in the `mem` network a peer is found by key.
//! Links: `meshtown_core::invite::{make, parse, redeem, store}`, `node::start_node`, `core/tests/common/invite_nodes.rs`.

use std::time::Duration;

use data_encoding::BASE32_NOPAD_NOCASE;
use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::invite::store::{IssueVerdict, Ledger};
use meshtown_core::invite::{self, ParsedInvite, Source};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::PeerAddr;
use meshtown_core::transport::net_mainline::unix_now;

#[path = "common/invite_nodes.rs"]
mod common;
use common::{loopback_addr, quick_opts, redeem_via, wait_until, ENTRY_LIMIT_SECS};

const LABEL: &str = "island";

mod string_made_by_the_issuer {
    use super::*;

    mod before_commit {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn document_does_not_know_the_secret_yet() {
            // given
            let issuer = setup().await;
            // when
            let issued = invite::make(&issuer.source(), None).expect("make must build a string");
            // then
            let parsed = invite::parse(&issued.text).unwrap();
            assert_eq!(
                issuer.verdict(&parsed).await,
                IssueVerdict::Unknown,
                "make doesn't touch the document"
            );
        }
    }

    mod after_commit {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn document_holds_the_issue_record() {
            // given
            let issuer = setup().await;
            // when
            let parsed = issuer.when_invite_is_issued().await;
            // then
            assert_eq!(
                issuer.verdict(&parsed).await,
                IssueVerdict::Grant,
                "commit must record the issue"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn carries_network_id_label_and_address_of_the_issuer() {
            // given
            let issuer = setup().await;
            // when
            let parsed = issuer.when_invite_is_issued().await;
            // then
            assert_eq!(parsed.head.network_id, issuer.key.subkeys().network_id);
            assert_eq!(parsed.head.label.as_deref(), Some(LABEL));
            assert_eq!(
                parsed.peers[0].id, issuer.a.id,
                "the admitter is first in the string"
            );
            assert!(
                parsed.warnings.is_empty(),
                "a fresh string must have no warnings: {:?}",
                parsed.warnings
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn carries_no_network_key() {
            // given
            let issuer = setup().await;
            // when
            let text = issuer.when_text_is_issued().await;
            // then
            let body = text
                .strip_prefix(invite::INVITE_PREFIX)
                .expect("prefix mesh-invite2-");
            let bytes = BASE32_NOPAD_NOCASE
                .decode(body.as_bytes())
                .expect("the string body is base32");
            let key = issuer.key.to_bytes();
            assert!(
                !bytes.windows(32).any(|w| w == key.as_slice()),
                "the invite string must not contain the network key bytes"
            );
        }
    }
}

mod newcomer_with_the_string {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gets_the_key_of_the_issuer() {
        // given
        let issuer = setup().await;
        let parsed = issuer.when_invite_is_issued().await;
        // when
        let got = issuer.when_newcomer_redeems(&parsed).await;
        // then
        let got = got.expect("a string with a fresh secret must yield the network key");
        assert_eq!(got.encode(), issuer.key.encode());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn becomes_a_member_without_dht() {
        // given
        let issuer = setup().await;
        let parsed = issuer.when_invite_is_issued().await;
        // when
        let newcomer = issuer.when_newcomer_enters(&parsed).await;
        // then
        let joined = wait_until(Duration::from_secs(20), || {
            issuer.a.members.is_member(newcomer.id) && newcomer.members.is_member(issuer.a.id)
        })
        .await;
        assert!(
            joined,
            "the nodes did not become members of each other within 20 s through the invite"
        );
        newcomer.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn same_string_second_time_is_refused_as_used() {
        // given
        let issuer = setup().await;
        let parsed = issuer.when_invite_is_issued().await;
        issuer
            .when_newcomer_redeems(&parsed)
            .await
            .expect("the first join must succeed");
        // when
        let second = issuer.when_newcomer_redeems(&parsed).await;
        // then
        assert!(
            matches!(second, Err(RedeemError::Used)),
            "repeating the same string gave {second:?}"
        );
    }
}

// ============================================================================================
// setup and helpers
// ============================================================================================

/// The admitter node A: a network key, the network label `LABEL`, a `mem` network without DHT.
struct Issuer {
    key: NetworkKey,
    a: RunningNode,
}

async fn setup() -> Issuer {
    let key = NetworkKey::generate().unwrap();
    let cfg = NodeConfig {
        network_label: Some(LABEL.to_string()),
        ..NodeConfig::for_loopback_test(key.clone())
    };
    let a = node::start_node(cfg).await.unwrap();
    Issuer { key, a }
}

impl Issuer {
    /// What `RunningNode::invite_source` (it is `pub(crate)`) builds from the public fields of the node.
    fn source(&self) -> Source {
        Source {
            stack: self.a.stack.clone(),
            members: self.a.members.clone(),
            key: self.a.network_key.clone(),
            kind: self.a.net_kind,
            reply_secret: self.a.reply_secret.clone(),
            label: self.a.network_label.clone(),
            domains: self.a.domains.clone(),
        }
    }

    /// A string as the console and the page hand it out: `make` → `commit`.
    async fn when_text_is_issued(&self) -> String {
        let issued = invite::make(&self.source(), None).expect("make must build a string");
        issued
            .commit()
            .await
            .expect("commit must record the issue in the document")
            .text
    }

    async fn when_invite_is_issued(&self) -> ParsedInvite {
        let text = self.when_text_is_issued().await;
        assert!(
            text.starts_with(invite::INVITE_PREFIX),
            "the string must start with {}",
            invite::INVITE_PREFIX
        );
        invite::parse(&text).expect("a string built by the node must parse")
    }

    /// The verdict of A's document for the secret from the string.
    async fn verdict(&self, parsed: &ParsedInvite) -> IssueVerdict {
        Ledger::of(&self.a.domains)
            .verdict(&parsed.head.secret.hash_hex(), unix_now())
            .await
    }

    /// The join address: a member's id from the string; the slot hint is appended by `KeyedMem`.
    fn entry_addr(&self, parsed: &ParsedInvite) -> PeerAddr {
        loopback_addr(parsed.peers[0].id, &self.a)
    }

    async fn when_newcomer_redeems(
        &self,
        parsed: &ParsedInvite,
    ) -> Result<NetworkKey, RedeemError> {
        let opts = quick_opts(&parsed.head, ENTRY_LIMIT_SECS);
        redeem_via(&parsed.head, &[self.entry_addr(parsed)], opts).await
    }

    /// The newcomer gets a key by the string and starts with it; its peer's address comes only from the string.
    async fn when_newcomer_enters(&self, parsed: &ParsedInvite) -> RunningNode {
        let key = self
            .when_newcomer_redeems(parsed)
            .await
            .expect("a string with a fresh secret must yield a key");
        let mut cfg = NodeConfig::for_loopback_test(key);
        cfg.initial_peers = vec![self.entry_addr(parsed)];
        node::start_node(cfg).await.unwrap()
    }
}
