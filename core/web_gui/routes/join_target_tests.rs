//! `place_of`: where an invite to a network goes, read from the slots of a `FakeHost`.
// helper: mounted from `join_target.rs`; `route_tests` of the same module reuse the fixture below.

use std::sync::Arc;

use super::{place_of, Place};
use crate::config::config_file::NetSlotId;
use crate::members::network_key::parse_user_input;
use crate::web_gui::control::ActionError;
use crate::web_gui::routes::invite_fixture::{members_reply, word_key};
use crate::web_gui::test_host::{config_with_key, slot_handle, FakeHost, SnapNode};
use crate::web_gui::{MemberError, MemberRow, NodeStatus, RefusalInfo};

/// A host over slots `(id, key)` and the node of every slot.
pub(super) struct Fx {
    pub host: Arc<FakeHost>,
    pub nodes: Vec<(String, Arc<SnapNode>)>,
}

pub(super) fn fx(slots: &[(&str, Option<&str>)]) -> Fx {
    let nodes: Vec<(String, Arc<SnapNode>)> = slots
        .iter()
        .map(|(id, _)| (id.to_string(), SnapNode::new(Default::default())))
        .collect();
    let handles = slots
        .iter()
        .zip(&nodes)
        .map(|((id, key), (_, node))| slot_handle(id, node.clone(), config_with_key(*key)))
        .collect();
    Fx {
        host: FakeHost::new(handles),
        nodes,
    }
}

impl Fx {
    pub fn node(&self, id: &str) -> &Arc<SnapNode> {
        &self.nodes.iter().find(|(n, _)| n == id).expect("a slot").1
    }

    pub fn exclude(&self, id: &str) {
        self.node(id).snapshot.lock().unwrap().members.me_excluded = true;
    }

    pub fn remove(&self, id: &str) {
        self.node(id).snapshot.lock().unwrap().node.status = NodeStatus::Offline;
    }
}

pub(super) fn id_hex(key_text: &str) -> String {
    parse_user_input(key_text)
        .expect("a key")
        .key
        .subkeys()
        .network_id_hex()
}

fn slot(id: &str) -> NetSlotId {
    NetSlotId::parse(id).expect("a slot id")
}

#[tokio::test]
async fn absent_with_main_empty_is_main() {
    let f = fx(&[("main", None)]);
    let place = place_of(f.host.as_ref(), &id_hex(&word_key("beta"))).await;
    assert_eq!(place, Place::Main);
}

#[tokio::test]
async fn absent_with_main_busy_is_fresh() {
    let a = word_key("alpha");
    let f = fx(&[("main", Some(&a))]);
    let place = place_of(f.host.as_ref(), &id_hex(&word_key("beta"))).await;
    assert_eq!(place, Place::Fresh);
}

#[tokio::test]
async fn active_is_here() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&word_key("alpha"))), ("ab2c3d", Some(&b))]);
    f.node("ab2c3d").set_members(Ok(members_reply()));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    let want = Place::Here {
        slot: slot("ab2c3d"),
        label: Some("beta".to_string()),
    };
    assert_eq!(place, want);
}

#[tokio::test]
async fn removed_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&word_key("alpha"))), ("ab2c3d", Some(&b))]);
    f.remove("ab2c3d");
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("ab2c3d")));
}

#[tokio::test]
async fn excluded_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    f.exclude("main");
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("main")));
}

#[tokio::test]
async fn unreadable_members_is_here() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    let unreadable = MemberError::Action(ActionError::NotFound("not running".to_string()));
    f.node("main").set_members(Err(unreadable));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert!(matches!(place, Place::Here { .. }), "{place:?}");
}

#[tokio::test]
async fn unreadable_members_of_an_excluded_node_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    f.exclude("main");
    let unreadable = MemberError::Action(ActionError::NotFound("not running".to_string()));
    f.node("main").set_members(Err(unreadable));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("main")));
}

#[tokio::test]
async fn an_offline_node_that_answers_nothing_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    let busy = MemberError::Action(ActionError::Busy("no network yet".to_string()));
    f.node("main").set_members(Err(busy));
    f.node("main").snapshot.lock().unwrap().node.status = NodeStatus::Offline;
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("main")));
}

fn row(
    is_self: bool,
    status: &'static str,
    refusal: Option<(&'static str, &'static str)>,
) -> MemberRow {
    MemberRow {
        id: String::new(),
        id_full: String::new(),
        name: None,
        is_self,
        online: true,
        admin: false,
        status,
        excluded: None,
        blocked_by_me: false,
        denied_by_me: None,
        last_seen_sec: None,
        last_refusal: refusal.map(|(side, reason)| RefusalInfo {
            side,
            reason,
            at_unix: 1,
        }),
        admission: None,
    }
}

fn set_rows(f: &Fx, id: &str, mine: &'static str, refusal: (&'static str, &'static str)) {
    let mut reply = members_reply();
    reply.members = vec![row(true, mine, None), row(false, "member", Some(refusal))];
    f.node(id).set_members(Ok(reply));
}

#[tokio::test]
async fn a_node_turned_away_as_unlisted_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    set_rows(&f, "main", "unlisted", ("them", "unlisted"));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("main")));
}

#[tokio::test]
async fn a_member_with_an_old_unlisted_refusal_is_here() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    set_rows(&f, "main", "member", ("them", "unlisted"));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert!(matches!(place, Place::Here { .. }), "{place:?}");
}

#[tokio::test]
async fn a_node_waiting_for_verdicts_is_here() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    let mut reply = members_reply();
    reply.members = vec![row(true, "unlisted", None), row(false, "member", None)];
    f.node("main").set_members(Ok(reply));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert!(matches!(place, Place::Here { .. }), "{place:?}");
}

#[tokio::test]
async fn a_refusal_from_this_side_is_here() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&b))]);
    set_rows(&f, "main", "unlisted", ("me", "unlisted"));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert!(matches!(place, Place::Here { .. }), "{place:?}");
}

#[tokio::test]
async fn an_unlisted_slot_beside_another_network_is_return() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&word_key("alpha"))), ("ab2c3d", Some(&b))]);
    f.node("main").set_members(Ok(members_reply()));
    set_rows(&f, "ab2c3d", "unlisted", ("them", "unlisted"));
    let place = place_of(f.host.as_ref(), &id_hex(&b)).await;
    assert_eq!(place, Place::Return(slot("ab2c3d")));
}
