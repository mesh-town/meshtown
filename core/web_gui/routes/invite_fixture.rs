//! Invite strings for route tests: fresh or expired invites of the network a word key stands for,
//! and the member-list replies a node gives about itself.

use crate::invite::{build_keyed, InviteHead, InviteSecret, INVITE_TTL_SECS};
use crate::members::network_key::{format_managed_key, parse_user_input};
use crate::members::words::generate_tail;
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{NodeSecret, PeerAddr, PeerId};
use crate::web_gui::members::dto::{Counts, MembersReply};
use crate::web_gui::now_unix;

/// The node that created every network of `word_key`.
const CREATOR: u8 = 7;

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn invite_made_at(key_text: &str, made_unix: u64) -> String {
    let input = parse_user_input(key_text).expect("a key the invite stands for");
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: input.key.subkeys().network_id,
        label: input.label,
        made_unix,
        root: input.root,
    };
    let members = [PeerAddr::new(peer(1)), PeerAddr::new(peer(2))];
    build_keyed(&head, &members[0], &members[1..], &PrimaryIps::NONE).expect("the invite is built")
}

/// A fresh invite to the network `key_text` is the key of.
pub(crate) fn invite_into(key_text: &str) -> String {
    invite_made_at(key_text, now_unix())
}

/// An invite to the same network that is older than its lifetime.
pub(crate) fn stale_invite_into(key_text: &str) -> String {
    let made = now_unix() - INVITE_TTL_SECS - 10;
    invite_made_at(key_text, made)
}

/// A new words key with the label, of a network created by a fixed node.
pub(crate) fn word_key(label: &str) -> String {
    let tail = generate_tail().expect("a random tail");
    format_managed_key(Some(label), &tail, &peer(CREATOR))
}

/// A member-list reply of a node that is a plain member of a network created by `CREATOR`.
pub(crate) fn members_reply() -> MembersReply {
    MembersReply {
        me: "me".to_string(),
        me_excluded: false,
        me_pending: false,
        admins: Vec::new(),
        me_admin: false,
        root: Some(peer(CREATOR).to_string()),
        me_admission: None,
        counts: Counts {
            members: 0,
            excluded: 0,
            unlisted: 0,
            pending: 0,
        },
        members: Vec::new(),
        changed: None,
    }
}
