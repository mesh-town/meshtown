//! Tests of the invite head for a Managed network: which key a redemption reply stands for and how the
//! key text is written (`head.rs`).

use super::*;
use crate::members::network_key::{
    managed_key, managed_key_with_label, parse_user_input, NetworkKey,
};
use crate::transport::net::{NodeSecret, PeerId};

mod reply_key {
    use super::*;

    mod in_a_managed_network {
        use super::*;

        #[test]
        fn is_derived_from_the_reply_and_the_creator() {
            // given
            let rig = setup();
            // when
            let key = rig.head.reply_key(&rig.reply);
            // then
            let want = managed_key(&rig.reply.to_bytes(), &rig.root);
            assert_eq!(key.to_bytes(), want.to_bytes());
            assert_ne!(
                key.to_bytes(),
                rig.reply.to_bytes(),
                "the reply is the base, not the key"
            );
        }

        #[test]
        fn has_the_network_id_of_the_head() {
            // given
            let rig = setup();
            // when
            let key = rig.head.reply_key(&rig.reply);
            // then
            assert_eq!(key.subkeys().network_id, rig.head.network_id);
        }
    }
}

mod key_text {
    use super::*;

    #[test]
    fn managed_parses_to_the_network_of_the_head() {
        // given
        let rig = setup();
        // when
        let text = rig.head.key_text(&rig.reply);
        let parsed = parse_user_input(&text).expect("a Managed key text parses");
        // then
        assert_eq!(parsed.root, rig.root);
        assert_eq!(parsed.key.subkeys().network_id, rig.head.network_id);
        assert_eq!(parsed.label.as_deref(), Some(LABEL));
    }

    #[test]
    fn key_text_is_the_mesh2_form() {
        // given
        let rig = setup();
        // when
        let text = rig.head.key_text(&rig.reply);
        // then
        let want = managed_key_with_label(Some(LABEL), &rig.reply.to_bytes(), &rig.root);
        assert_eq!(text, want);
        assert!(text.starts_with(&format!("{LABEL}--mesh2-")), "{text}");
    }

    #[test]
    fn managed_is_the_same_network_for_the_head() {
        // given
        let rig = setup();
        // when
        let text = rig.head.key_text(&rig.reply);
        // then
        assert!(same_network(&rig.head, &text).expect("the text parses"));
    }
}

mod swapped_base {
    use super::*;

    #[test]
    fn other_base_is_another_network() {
        // given: a Managed head and a reply with another base
        let rig = setup();
        let other = NetworkKey::from_bytes([0x66; 32]);
        // when
        let key = rig.head.reply_key(&other);
        // then
        assert_ne!(
            key.subkeys().network_id,
            rig.head.network_id,
            "a reply of another base must fail the check against the head"
        );
    }

    #[test]
    fn other_creator_is_another_network() {
        // given: the same reply, but the head names another creator
        let rig = setup();
        let mut head = rig.head.clone();
        head.root = peer(9);
        // when
        let key = head.reply_key(&rig.reply);
        // then
        assert_ne!(key.subkeys().network_id, rig.head.network_id);
    }

    #[test]
    fn the_unmodified_reply_passes_the_check() {
        // given
        let rig = setup();
        // when
        let key = rig.head.reply_key(&rig.reply);
        // then
        assert_eq!(key.subkeys().network_id, rig.head.network_id);
    }
}

const LABEL: &str = "work";

struct Rig {
    head: InviteHead,
    /// What a redemption answers with: the base of the network key.
    reply: NetworkKey,
    root: PeerId,
}

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A head of a network that was made from `reply`.
fn setup() -> Rig {
    let reply = NetworkKey::from_bytes([0x42; 32]);
    let root = peer(7);
    let key = managed_key(&reply.to_bytes(), &root);
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: key.subkeys().network_id,
        label: Some(LABEL.to_string()),
        made_unix: 1_760_000_000,
        root,
    };
    Rig { head, reply, root }
}
