use meshtown_core::transport::net::NodeSecret;
use meshtown_core::web_gui::{MemberActionReq as Act, MemberError};

use super::*;
use crate::members_reply::{build_reply, demo_self_id_full};
use crate::node::{DemoNetworkOptions, DemoPeer};

fn peer(seed: u8, name: &str) -> DemoPeer {
    let id = NodeSecret::from_bytes(&[seed; 32]).peer_id();
    DemoPeer {
        id: id.to_string()[..8].to_string(),
        id_full: id,
        name: Some(name.to_string()),
        path: None,
        online: true,
        exit: true,
    }
}

fn demo(mode: ManagedMode) -> DemoMembers {
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
        me_removed: false,
        managed: mode,
    };
    DemoMembers::new(&[peer(2, "peer-one"), peer(3, "peer-two")], &opts)
}

fn id_of(m: &DemoMembers, name: &str) -> String {
    let e = m.entries.iter().find(|e| e.name.as_deref() == Some(name));
    e.unwrap().id_full.clone()
}

fn code(res: Result<bool, MemberError>) -> &'static str {
    match res {
        Err(MemberError::Refused { code, .. }) => code,
        Ok(_) => "ok",
        Err(_) => "other",
    }
}

#[test]
fn peer_mode_has_no_roles_and_refuses_role_actions() {
    let mut m = demo(ManagedMode::Peer);
    let one = id_of(&m, "peer-one");
    assert!(m.roles.is_none());
    assert_eq!(code(m.apply(&Act::Grant(one.clone()))), "not_managed");
    assert_eq!(code(m.apply(&Act::Revoke(one))), "not_managed");
    assert_eq!(code(m.apply(&Act::Close)), "not_managed");
    let reply = build_reply(&m, "abcd1234", None);
    assert_eq!(
        (reply.kind, reply.root, reply.me_admin),
        ("Peer", None, false)
    );
    assert!(reply.admins.is_empty() && reply.members.iter().all(|r| !r.admin));
}

#[test]
fn admin_mode_makes_me_the_root_with_one_more_admin() {
    let m = demo(ManagedMode::Admin);
    let reply = build_reply(&m, "abcd1234", None);
    let me = demo_self_id_full("abcd1234");
    assert_eq!(reply.kind, "Managed");
    assert_eq!(reply.root.as_deref(), Some(me.as_str()));
    assert!(reply.me_admin);
    let mut want = vec![me, id_of(&m, "peer-one")];
    want.sort();
    assert_eq!(reply.admins, want);
    let flagged: Vec<&str> = reply
        .members
        .iter()
        .filter(|r| r.admin)
        .map(|r| r.id_full.as_str())
        .collect();
    assert_eq!(flagged.len(), 2);
}

#[test]
fn member_mode_leaves_two_other_admins() {
    let m = demo(ManagedMode::Member);
    let reply = build_reply(&m, "abcd1234", None);
    assert!(!reply.me_admin);
    assert_eq!(reply.root.as_deref(), Some(id_of(&m, "peer-one").as_str()));
    assert!(reply.admins.contains(&id_of(&m, "peer-two")));
    assert_eq!(reply.admins.len(), 2);
}

#[test]
fn a_plain_member_cannot_use_power_actions() {
    let mut m = demo(ManagedMode::Member);
    let two = id_of(&m, "peer-two");
    assert_eq!(code(m.apply(&Act::Grant(two.clone()))), "not_admin");
    assert_eq!(code(m.apply(&Act::Revoke(two))), "not_admin");
    assert_eq!(code(m.apply(&Act::Close)), "not_admin");
}

#[test]
fn grant_and_revoke_change_the_admins() {
    let mut m = demo(ManagedMode::Admin);
    let two = id_of(&m, "peer-two");
    assert_eq!(code(m.apply(&Act::Grant(two.clone()))), "ok");
    assert!(m.is_admin(&two));
    assert!(!m.apply(&Act::Grant(two.clone())).unwrap());
    assert!(m.apply(&Act::Revoke(two.clone())).unwrap());
    assert!(!m.is_admin(&two));
}

#[test]
fn the_root_keeps_the_role() {
    let mut m = demo(ManagedMode::Admin);
    let me = demo_self_id_full("abcd1234");
    let act = crate::managed::own_id_as_me(Act::Revoke(me), "abcd1234");
    assert_eq!(code(m.apply(&act)), "not_admin");
}

#[test]
fn the_last_admin_cannot_give_up_or_leave() {
    let mut m = demo(ManagedMode::Admin);
    let one = id_of(&m, "peer-one");
    m.apply(&Act::Revoke(one)).unwrap();
    assert_eq!(code(m.apply(&Act::Leave)), "last_admin");
    let mut r = m.roles.take().unwrap();
    r.root = id_of(&m, "peer-one");
    m.roles = Some(r);
    let act = crate::managed::own_id_as_me(Act::Revoke(demo_self_id_full("abcd1234")), "abcd1234");
    assert_eq!(code(m.apply(&act)), "last_admin");
}

#[test]
fn grant_needs_a_listed_member() {
    let mut m = demo(ManagedMode::Admin);
    let gone = id_of(&m, "peer-three");
    assert_eq!(code(m.apply(&Act::Grant(gone))), "not_member");
}

#[test]
fn forgetting_is_peer_only() {
    let mut m = demo(ManagedMode::Admin);
    let one = id_of(&m, "peer-one");
    assert_eq!(code(m.apply(&Act::Forget(one.clone()))), "peer_only");
    assert_eq!(code(m.apply(&Act::Unforget(one))), "peer_only");
}

#[test]
fn close_empties_the_list_and_refuses_everything() {
    let mut m = demo(ManagedMode::Admin);
    assert_eq!(code(m.apply(&Act::Close)), "ok");
    let one = id_of(&m, "peer-one");
    assert_eq!(code(m.apply(&Act::Grant(one.clone()))), "network_deleted");
    assert_eq!(code(m.apply(&Act::Block(one))), "network_deleted");
    assert_eq!(code(m.apply(&Act::Leave)), "network_deleted");
    let reply = build_reply(&m, "abcd1234", None);
    assert!(reply.members.is_empty());
    assert_eq!(reply.counts.members, 0);
}
