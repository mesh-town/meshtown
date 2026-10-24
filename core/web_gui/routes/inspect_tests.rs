//! `POST /api/network/inspect`: what a key or an invite string is — the label, the kind of the network and its
//! id — without joining anything and without spending the invite.

// helper: mounted from `inspect.rs` as `#[cfg(test)] #[path = "inspect_tests.rs"] mod tests;`.
// The tests go through the real router over `FakeHost` and `SnapNode`. `SnapNode` cannot count `join_by_invite`
// or `switch_network` calls, so "nothing happened" is read from the host calls, the key of the slot, the node's
// restart and invite counters.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::invite::{build_keyed, InviteHead, InviteSecret};
use crate::members::network_key::{
    format_managed_key, key_with_label, managed_key, managed_key_with_label, parse_user_input,
    NetKind, NetworkKey,
};
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{NodeSecret, PeerAddr, PeerId};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::{now_unix, NodeControl, SlotHandle};

const NETWORK_ID: [u8; 4] = [0xaa, 0xbb, 0xcc, 0xdd];
const NETWORK_ID_HEX: &str = "aabbccdd";
const BASE: [u8; 32] = [9; 32];

mod an_invite {
    use super::*;

    #[tokio::test]
    async fn of_a_managed_network_says_managed_with_label_and_id() {
        // given
        let f = setup(None);
        let invite = invite_of(NetKind::Managed { root: peer(7) }, Some("work"));
        // when
        let (status, reply) = f.when_inspect(&invite).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Managed", "{reply}");
        assert_eq!(reply["label"], "work", "{reply}");
        assert_eq!(reply["id_hex"], NETWORK_ID_HEX, "{reply}");
    }

    #[tokio::test]
    async fn of_a_peer_network_says_peer_with_label_and_id() {
        // given
        let f = setup(None);
        let invite = invite_of(NetKind::Peer, Some("home"));
        // when
        let (status, reply) = f.when_inspect(&invite).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Peer", "{reply}");
        assert_eq!(reply["label"], "home", "{reply}");
        assert_eq!(reply["id_hex"], NETWORK_ID_HEX, "{reply}");
    }

    #[tokio::test]
    async fn without_a_label_has_a_null_label() {
        // given
        let f = setup(None);
        let invite = invite_of(NetKind::Managed { root: peer(7) }, None);
        // when
        let (status, reply) = f.when_inspect(&invite).await;
        // then
        // assumed: no label in the invite is `null` in the reply, not an empty string.
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["label"], Value::Null, "{reply}");
    }
}

mod a_key {
    use super::*;

    #[tokio::test]
    async fn in_the_peer_raw_form_says_peer() {
        // given
        let f = setup(None);
        let key = NetworkKey::from_bytes(BASE);
        // when
        let (status, reply) = f.when_inspect(&key_with_label(Some("home"), &key)).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Peer", "{reply}");
        assert_eq!(reply["label"], "home", "{reply}");
        assert_eq!(reply["id_hex"], key.subkeys().network_id_hex(), "{reply}");
    }

    #[tokio::test]
    async fn without_a_label_says_peer_with_a_null_label() {
        // given
        let f = setup(None);
        let key = NetworkKey::from_bytes(BASE);
        // when
        let (status, reply) = f.when_inspect(&key.encode()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Peer", "{reply}");
        assert_eq!(reply["label"], Value::Null, "{reply}");
    }

    #[tokio::test]
    async fn in_the_managed_words_form_says_managed() {
        // given
        let f = setup(None);
        let tail = crate::members::words::generate_tail().expect("a tail");
        let key = format_managed_key(Some("work"), &tail, &peer(7));
        // when
        let (status, reply) = f.when_inspect(&key).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Managed", "{reply}");
        assert_eq!(reply["label"], "work", "{reply}");
        let want = parse_user_input(&key).expect("the key parses");
        assert_eq!(
            reply["id_hex"],
            want.key.subkeys().network_id_hex(),
            "{reply}"
        );
    }

    #[tokio::test]
    async fn in_the_managed_raw_form_says_managed() {
        // given
        let f = setup(None);
        let key = managed_key_with_label(Some("work"), &BASE, &peer(7));
        // when
        let (status, reply) = f.when_inspect(&key).await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(reply["kind"], "Managed", "{reply}");
        assert_eq!(reply["label"], "work", "{reply}");
        let want = managed_key(&BASE, &peer(7)).subkeys().network_id_hex();
        assert_eq!(reply["id_hex"], want, "{reply}");
    }
}

mod garbage {
    use super::*;

    #[tokio::test]
    async fn text_that_is_no_key_is_refused_as_invalid_key() {
        for text in ["hello world", "mesh1-", "work--not-a-key-at-all"] {
            // given
            let f = setup(None);
            // when
            let (status, reply) = f.when_inspect(text).await;
            // then
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{text}: {reply}");
            assert_eq!(reply["error"], "invalid_key", "{text}: {reply}");
        }
    }

    #[tokio::test]
    async fn a_broken_invite_is_refused_as_invalid_key() {
        // given: a string that starts like an invite and is cut short
        let f = setup(None);
        let invite = invite_of(NetKind::Peer, Some("home"));
        let cut = &invite[..invite.len() / 2];
        // when
        let (status, reply) = f.when_inspect(cut).await;
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
        assert_eq!(reply["error"], "invalid_key", "{reply}");
    }
}

mod inspecting {
    use super::*;

    #[tokio::test]
    async fn an_invite_leaves_the_slot_the_host_and_the_node_as_they_were() {
        // given
        let f = setup(None);
        let invite = invite_of(NetKind::Managed { root: peer(7) }, Some("work"));
        // when
        f.when_inspect(&invite).await;
        // then
        f.then_nothing_changed(None);
    }

    #[tokio::test]
    async fn a_key_leaves_the_slot_the_host_and_the_node_as_they_were() {
        // given
        let f = setup(None);
        let key = key_with_label(Some("home"), &NetworkKey::from_bytes(BASE));
        // when
        f.when_inspect(&key).await;
        // then
        f.then_nothing_changed(None);
    }

    #[tokio::test]
    async fn garbage_leaves_them_as_they_were_too() {
        // given
        let f = setup(None);
        // when
        f.when_inspect("hello world").await;
        // then
        f.then_nothing_changed(None);
    }

    #[tokio::test]
    async fn on_a_slot_with_a_network_answers_and_keeps_that_network() {
        // given
        let own = key_with_label(Some("own"), &NetworkKey::from_bytes([5; 32]));
        let f = setup(Some(own.clone()));
        let invite = invite_of(NetKind::Managed { root: peer(7) }, Some("work"));
        // when
        let (status, reply) = f.when_inspect(&invite).await;
        // then
        // assumed: unlike `join`, inspecting is allowed while the slot has a network.
        assert_eq!(status, StatusCode::OK, "{reply}");
        f.then_nothing_changed(Some(own));
    }

    #[tokio::test]
    async fn the_same_invite_gives_the_same_answer_twice() {
        // given
        let f = setup(None);
        let invite = invite_of(NetKind::Managed { root: peer(7) }, Some("work"));
        // when
        let first = f.when_inspect(&invite).await;
        let second = f.when_inspect(&invite).await;
        // then: looking at the invite does not use it up
        assert_eq!(first, second);
        assert_eq!(second.0, StatusCode::OK, "{}", second.1);
    }
}

// --- helpers ---

struct Fixture {
    router: axum::Router,
    host: Arc<FakeHost>,
    node: Arc<SnapNode>,
    slot: SlotHandle,
}

/// One slot `main`, with the given network key or with none.
fn setup(network_key: Option<String>) -> Fixture {
    let node = SnapNode::new(Default::default());
    let slot = slot_handle(
        "main",
        node.clone(),
        config_with_key(network_key.as_deref()),
    );
    let host = FakeHost::new(vec![slot.clone()]);
    Fixture {
        router: test_app(host.clone()).0,
        host,
        node,
        slot,
    }
}

impl Fixture {
    async fn when_inspect(&self, text: &str) -> (StatusCode, Value) {
        let body = json!({"key": text}).to_string();
        send(&self.router, Method::POST, "/api/network/inspect", &body).await
    }

    /// The host got no calls, the slot still has `key`, the node was neither restarted nor made to wait for an invite.
    fn then_nothing_changed(&self, key: Option<String>) {
        assert_eq!(self.host.calls(), vec![], "the host was not touched");
        assert_eq!(self.slot.config.lock().unwrap().current_network_key, key);
        assert_eq!(self.node.restarts(), 0, "the node was not restarted");
        assert!(
            self.node.invite_join().is_none(),
            "no invite is being redeemed"
        );
    }
}

/// An invite string for a network of the given kind, fresh now.
fn invite_of(kind: NetKind, label: Option<&str>) -> String {
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: NETWORK_ID,
        label: label.map(str::to_string),
        made_unix: now_unix(),
        kind,
    };
    let members = [PeerAddr::new(peer(1)), PeerAddr::new(peer(2))];
    build_keyed(&head, &members[0], &members[1..], &PrimaryIps::NONE).expect("the invite is built")
}

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}
