//! `opened_locals_of`: the `local` ports other slots have opened, each with the name of its network.

use super::*;
use crate::members::network_key::{format_managed_key, parse_user_input};
use crate::members::words::generate_tail;
use crate::ports_sharing::{Opened, Proto};
use crate::transport::net::NodeSecret;
use crate::web_gui::test_host::{config_with_key, slot_handle, SnapNode};

const A: &str = "ab2c3d";
const B: &str = "bc3d4e";
const C: &str = "cd4e5f";

mod two_slots_with_keys {
    use super::*;

    #[test]
    fn lists_the_locals_of_the_other_slot_under_its_label() {
        // given: the slot B opens `local 9000`
        let key_a = key_with_label("home");
        let key_b = key_with_label("work");
        let slots = vec![slot(A, Some(&key_a), &[]), slot(B, Some(&key_b), &[9000])];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        assert_eq!(got, vec![local(9000, "work")]);
    }

    #[test]
    fn skips_the_locals_of_the_slot_that_is_excepted() {
        // given: the slot A opens `local 9000`
        let key_a = key_with_label("home");
        let key_b = key_with_label("work");
        let slots = vec![slot(A, Some(&key_a), &[9000]), slot(B, Some(&key_b), &[])];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        assert!(got.is_empty(), "got {got:?}");
    }

    #[test]
    fn lists_every_local_of_every_other_slot() {
        // given: B and C open two locals and one local
        let slots = vec![
            slot(A, Some(&key_with_label("home")), &[9100]),
            slot(B, Some(&key_with_label("work")), &[9000, 9001]),
            slot(C, Some(&key_with_label("play")), &[9002]),
        ];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        let expected = vec![
            local(9000, "work"),
            local(9001, "work"),
            local(9002, "play"),
        ];
        assert_eq!(got, expected);
    }
}

mod a_slot_without_a_key {
    use super::*;

    #[test]
    fn is_skipped_even_if_its_settings_hold_opened_ports() {
        // given
        let slots = vec![
            slot(A, Some(&key_with_label("home")), &[]),
            slot(B, None, &[9000]),
        ];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        assert!(got.is_empty(), "got {got:?}");
    }
}

mod the_network_name {
    use super::*;

    #[test]
    fn without_a_label_is_network_and_the_first_four_characters_of_the_id() {
        // given: the key of B has no label
        let key_b = format_managed_key(None, &generate_tail().unwrap(), &creator());
        let slots = vec![
            slot(A, Some(&key_with_label("home")), &[]),
            slot(B, Some(&key_b), &[9000]),
        ];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        let expected = format!("Network {}", id_head(&key_b));
        assert_eq!(got, vec![local(9000, &expected)]);
        assert_eq!(expected.chars().count(), "Network ".len() + 4);
    }

    #[test]
    fn of_a_key_that_does_not_parse_is_the_id_of_the_slot() {
        // given
        let slots = vec![
            slot(A, Some(&key_with_label("home")), &[]),
            slot(B, Some("not-a-key"), &[9000]),
        ];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        assert_eq!(got, vec![local(9000, B)]);
    }
}

mod the_protocol {
    use super::*;

    #[test]
    fn keeps_the_protocol_of_each_local() {
        // given: B opens `local 9000` over UDP
        let slots = vec![
            slot(A, Some(&key_with_label("home")), &[]),
            udp_slot(B, Some(&key_with_label("work")), 9000),
        ];
        // when
        let got = opened_locals_of(slots, &id(A));
        // then
        let expected = OpenedLocal {
            local: 9000,
            proto: Proto::Udp,
            net: "work".to_string(),
        };
        assert_eq!(got, vec![expected]);
    }
}

fn creator() -> crate::transport::net::PeerId {
    NodeSecret::from_bytes(&[7; 32]).peer_id()
}

fn id(text: &str) -> NetSlotId {
    NetSlotId::parse(text).expect("a valid slot id")
}

fn key_with_label(label: &str) -> String {
    format_managed_key(Some(label), &generate_tail().unwrap(), &creator())
}

/// The first four characters of the network id a key stands for.
fn id_head(key: &str) -> String {
    let id_hex = parse_user_input(key)
        .unwrap()
        .key
        .subkeys()
        .network_id_hex();
    id_hex.chars().take(4).collect()
}

fn local(local: u16, net: &str) -> OpenedLocal {
    OpenedLocal {
        local,
        proto: Proto::Tcp,
        net: net.to_string(),
    }
}

/// A slot working with `key` (if any) whose settings open one local UDP port.
fn udp_slot(slot_id: &str, key: Option<&str>, local: u16) -> SlotHandle {
    let node = NodeSecret::from_bytes(&[5u8; 32]).peer_id();
    let mut state = config_with_key(key);
    state.file_settings.net.ports_opened = vec![Opened {
        proto: Proto::Udp,
        node,
        port: 53,
        local,
    }];
    slot_handle(slot_id, SnapNode::new(Default::default()), state)
}

/// A slot working with `key` (if any) whose settings open the given local ports.
fn slot(slot_id: &str, key: Option<&str>, locals: &[u16]) -> SlotHandle {
    let node = NodeSecret::from_bytes(&[5u8; 32]).peer_id();
    let mut state = config_with_key(key);
    state.file_settings.net.ports_opened = locals
        .iter()
        .map(|&local| Opened {
            proto: Proto::Tcp,
            node,
            port: 80,
            local,
        })
        .collect();
    slot_handle(slot_id, SnapNode::new(Default::default()), state)
}
