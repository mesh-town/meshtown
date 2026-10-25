use std::sync::{Arc, Mutex};

use clap::Parser;
use meshtown_core::config::config_file::Settings;
use meshtown_core::invite::parse;
use meshtown_core::members::network_key::{format_key_string, parse_user_input};
use meshtown_core::members::words::generate_tail;
use meshtown_core::web_gui::{ActionError, NodeControl};

use super::remember;
use crate::args::Args;
use crate::node::{DemoNetworkOptions, DemoNode};

fn node_with(key: Option<String>) -> DemoNode {
    let args = Args::parse_from(["web_demo", "--no-network"]);
    let mut settings = Settings::default();
    settings.net.network_key = key;
    let opts = DemoNetworkOptions {
        offline_peers: 0,
        no_exit_peers: 0,
        admission_refused: false,
        dht_joined: false,
        dht_nodes: 0,
        members_second_vote: false,
        me_excluded: false,
        members_pending: false,
        me_pending: false,
        members_forget: false,
        me_removed: true,
        managed: Default::default(),
    };
    DemoNode::new(1, args.fast, 100, true, opts, settings)
}

fn new_key(label: &str) -> String {
    format_key_string(Some(label), &generate_tail().expect("a key tail"))
}

#[test]
fn a_known_network_is_entered_and_saved() {
    let key = new_key("known-one");
    remember(&key);
    let invite = parse(&super::invite_for(&key).expect("invite")).expect("parsed");
    let node = node_with(None);
    let saved = Arc::new(Mutex::new(None));
    let sink = saved.clone();

    let result = node.join_by_invite(
        invite.head,
        invite.peers,
        Box::new(move |k| {
            *sink.lock().unwrap() = Some(k.to_string());
            Ok(())
        }),
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(saved.lock().unwrap().as_deref(), Some(key.as_str()));
    let own = parse(&node.invite().expect("invite of the entered network")).expect("parsed");
    assert_eq!(own.head.label.as_deref(), Some("known-one"));
    assert!(!node.state.lock().unwrap().members.me_removed);
}

#[test]
fn an_unknown_network_is_refused() {
    let key = new_key("never-seen");
    let invite = parse(&super::invite_for(&key).expect("invite")).expect("parsed");
    let node = node_with(None);

    let result = node.join_by_invite(invite.head, invite.peers, Box::new(|_| Ok(())));

    assert!(matches!(result, Err(ActionError::Invalid(_))), "{result:?}");
}

#[test]
fn the_invite_of_a_slot_carries_its_network() {
    let key = new_key("slot-net");
    let node = node_with(Some(key.clone()));

    let invite = parse(&node.invite().expect("invite")).expect("parsed");

    let id = parse_user_input(&key)
        .expect("key")
        .key
        .subkeys()
        .network_id;
    assert_eq!(invite.head.network_id, id);
}
