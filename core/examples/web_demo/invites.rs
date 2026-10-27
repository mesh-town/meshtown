//! Invites in the demo: the networks the demo knows by id, an invite built from a slot's key, and the
//! entry of a node into a known network.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use meshtown_core::invite::{build_keyed, InviteHead, InviteSecret};
use meshtown_core::members::network_key::{managed_key_with_label, parse_user_input};
use meshtown_core::transport::addr_order::PrimaryIps;
use meshtown_core::transport::net::{AddrHint, NodeSecret, PeerAddr};
use meshtown_core::web_gui::{ActionError, NodeControl, SaveKey};

use crate::node::DemoNode;

/// The network every demo invite falls back to when the slot has no key of its own.
const FALLBACK_LABEL: &str = "demo-invited";

fn registry() -> &'static Mutex<HashMap<[u8; 4], String>> {
    static KNOWN: OnceLock<Mutex<HashMap<[u8; 4], String>>> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let key = fallback_key();
        let id = parse_user_input(&key)
            .expect("the fallback key parses")
            .key
            .subkeys()
            .network_id;
        Mutex::new(HashMap::from([(id, key)]))
    })
}

/// The fake member that signs every demo invite; it is also the creator of the fallback network.
fn inviter() -> NodeSecret {
    NodeSecret::from_bytes(&[0x01u8; 32])
}

fn fallback_key() -> String {
    managed_key_with_label(Some(FALLBACK_LABEL), &[0x42; 32], &inviter().peer_id())
}

/// Makes the network of `key_text` known: an invite to it can be entered. Text that is no key is ignored.
pub(crate) fn remember(key_text: &str) {
    let Ok(input) = parse_user_input(key_text) else {
        return;
    };
    let id = input.key.subkeys().network_id;
    registry().lock().unwrap().insert(id, key_text.to_string());
}

/// The key text of the known network with this id.
pub(crate) fn key_of(network_id: [u8; 4]) -> Option<String> {
    registry().lock().unwrap().get(&network_id).cloned()
}

/// An invite to the network `key_text` stands for, signed by a fake member.
pub(crate) fn invite_for(key_text: &str) -> Result<String, ActionError> {
    let input = parse_user_input(key_text).map_err(|e| ActionError::Internal(e.to_string()))?;
    let mut me = PeerAddr::new(inviter().peer_id());
    me.hints.push(AddrHint::Ip(std::net::SocketAddr::from((
        [127, 0, 0, 1],
        4000,
    ))));
    let made_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x42; 32]),
        network_id: input.key.subkeys().network_id,
        label: input.label,
        made_unix,
        root: input.root,
    };
    build_keyed(&head, &me, &[], &PrimaryIps::NONE)
        .map_err(|e| ActionError::Internal(e.to_string()))
}

/// The key of the network this node is in; the fallback network when it has none.
pub(crate) fn key_of_node(node: &DemoNode) -> String {
    let own = node.state.lock().unwrap().settings.net.network_key.clone();
    own.unwrap_or_else(fallback_key)
}

/// Enters the known network of `head`: the key is saved, the node restarts into it and is a plain
/// member again (no ban).
pub(crate) fn enter(node: &DemoNode, head: &InviteHead, save: SaveKey) -> Result<(), ActionError> {
    let Some(key) = key_of(head.network_id) else {
        return Err(ActionError::Invalid(
            "the demo does not know this network".to_string(),
        ));
    };
    if let Err(e) = save(&key, None) {
        let t = node.state.lock().unwrap().t;
        node.push_log(t, "node", format!("the network key was not saved: {e}"));
    }
    node.switch_network(&key);
    let mut st = node.state.lock().unwrap();
    st.settings.net.network_key = Some(key);
    st.members.me_excluded = false;
    Ok(())
}

#[cfg(test)]
#[path = "invites_tests.rs"]
mod tests;
