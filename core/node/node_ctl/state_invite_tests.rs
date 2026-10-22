use super::*;
use crate::invite::store::{Ledger, IssueVerdict};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn string_does_not_carry_the_network_key() {
    // given
    let rig = setup().await;
    // when
    let invite = rig.when_invite_is_finished().await;
    // then
    rig.then_string_has_no_key(&invite);
    rig.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn issue_is_written_to_the_network_document_before_the_string_is_given() {
    // given
    let rig = setup().await;
    // when
    let invite = rig.when_invite_is_finished().await;
    // then
    let hash = crate::invite::parse(&invite)
        .unwrap()
        .head
        .secret
        .hash_hex();
    let verdict = Ledger::of(&rig.node.domains)
        .verdict(&hash, crate::net_mainline::unix_now())
        .await;
    assert_eq!(verdict, IssueVerdict::Grant, "the issue is not in the network document");
    rig.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_invites_have_different_secrets() {
    // given
    let rig = setup().await;
    // when
    let first = rig.when_invite_is_finished().await;
    let second = rig.when_invite_is_finished().await;
    // then
    let secret = |s: &str| crate::invite::parse(s).unwrap().head.secret;
    assert_ne!(
        secret(&first),
        secret(&second),
        "one-time secrets matched"
    );
    rig.shutdown().await;
}

struct Rig {
    ctl: std::sync::Arc<NodeCtl>,
    key: NetworkKey,
    node: RunningNode,
}

impl Rig {
    async fn when_invite_is_finished(&self) -> String {
        self.ctl
            .invite_pending(None)
            .expect("invite_pending() of a node on the network")
            .finish()
            .await
            .expect("the issue record in the network document succeeded")
    }

    /// The network key is not in the string, neither as byte pieces (8 in a row) nor as text.
    fn then_string_has_no_key(&self, invite: &str) {
        let body = data_encoding::BASE32_NOPAD_NOCASE
            .decode(&invite.as_bytes()[crate::invite::INVITE_PREFIX.len()..])
            .expect("the string body is base32");
        let bytes = self.key.to_bytes();
        let leaked = bytes
            .windows(8)
            .any(|piece| body.windows(8).any(|w| w == piece));
        assert!(!leaked, "a piece of the network key is in the invite body");
        assert!(
            !invite.contains(&self.key.encode()),
            "the key mesh1-… is in the string"
        );
    }

    async fn shutdown(self) {
        self.ctl.detach(NodeStatus::Stopping);
        self.node.shutdown().await;
    }
}

async fn setup() -> Rig {
    let ctl = NodeCtl::new(&Settings::default());
    let key = NetworkKey::generate().unwrap();
    let node = crate::node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    ctl.attach(&node);
    Rig { ctl, key, node }
}
