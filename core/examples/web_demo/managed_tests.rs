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

fn demo(mode: DemoRole) -> DemoMembers {
    let opts = DemoNetworkOptions {
        offline_peers: 0,
        no_exit_peers: 0,
        admission_refused: false,
        dht_joined: false,
        dht_nodes: 0,
        me_excluded: false,
        members_pending: true,
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
fn default_mode_makes_me_the_only_admin() {
    let m = demo(DemoRole::Root);
    let reply = build_reply(&m, "abcd1234", None);
    let me = demo_self_id_full("abcd1234");
    assert_eq!(reply.root.as_deref(), Some(me.as_str()));
    assert!(reply.me_admin);
    assert_eq!(reply.admins, [me]);
    assert_eq!(reply.members.iter().filter(|r| r.admin).count(), 1);
}

#[test]
fn second_admin_adds_the_first_peer() {
    let m = demo(DemoRole::SecondAdmin);
    let reply = build_reply(&m, "abcd1234", None);
    let me = demo_self_id_full("abcd1234");
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
    let m = demo(DemoRole::Member);
    let reply = build_reply(&m, "abcd1234", None);
    assert!(!reply.me_admin);
    assert_eq!(reply.root.as_deref(), Some(id_of(&m, "peer-one").as_str()));
    assert!(reply.admins.contains(&id_of(&m, "peer-two")));
    assert_eq!(reply.admins.len(), 2);
}

#[test]
fn a_plain_member_cannot_use_power_actions() {
    let mut m = demo(DemoRole::Member);
    let two = id_of(&m, "peer-two");
    assert_eq!(code(m.apply(&Act::Grant(two.clone()))), "not_admin");
    assert_eq!(code(m.apply(&Act::Revoke(two))), "not_admin");
    assert_eq!(code(m.apply(&Act::Close)), "not_admin");
}

#[test]
fn grant_and_revoke_change_the_admins() {
    let mut m = demo(DemoRole::Root);
    let two = id_of(&m, "peer-two");
    assert_eq!(code(m.apply(&Act::Grant(two.clone()))), "ok");
    assert!(m.is_admin(&two));
    assert!(!m.apply(&Act::Grant(two.clone())).unwrap());
    assert!(m.apply(&Act::Revoke(two.clone())).unwrap());
    assert!(!m.is_admin(&two));
}

#[test]
fn the_root_keeps_the_role() {
    let mut m = demo(DemoRole::Root);
    let me = demo_self_id_full("abcd1234");
    let act = crate::managed::own_id_as_me(Act::Revoke(me), "abcd1234");
    assert_eq!(code(m.apply(&act)), "not_admin");
}

#[test]
fn the_last_admin_cannot_give_up_or_leave() {
    let mut m = demo(DemoRole::Root);
    let one = id_of(&m, "peer-one");
    m.apply(&Act::Revoke(one)).unwrap();
    assert_eq!(code(m.apply(&Act::Leave)), "last_admin");
    m.roles.root = id_of(&m, "peer-one");
    let act = crate::managed::own_id_as_me(Act::Revoke(demo_self_id_full("abcd1234")), "abcd1234");
    assert_eq!(code(m.apply(&act)), "last_admin");
}

#[test]
fn grant_needs_a_listed_member() {
    let mut m = demo(DemoRole::Root);
    let gone = id_of(&m, "peer-three");
    assert_eq!(code(m.apply(&Act::Grant(gone))), "not_member");
}

#[test]
fn close_empties_the_list_and_refuses_everything() {
    let mut m = demo(DemoRole::Root);
    assert_eq!(code(m.apply(&Act::Close)), "ok");
    let one = id_of(&m, "peer-one");
    assert_eq!(code(m.apply(&Act::Grant(one.clone()))), "network_deleted");
    assert_eq!(code(m.apply(&Act::Block(one))), "network_deleted");
    assert_eq!(code(m.apply(&Act::Leave)), "network_deleted");
    let reply = build_reply(&m, "abcd1234", None);
    assert!(reply.members.is_empty());
    assert_eq!(reply.counts.members, 0);
}

#[test]
fn an_exclusion_by_an_admin_takes_effect_at_once() {
    let mut m = demo(DemoRole::Root);
    let two = id_of(&m, "peer-two");
    assert_eq!(
        code(m.apply(&Act::Exclude {
            id: two.clone(),
            reason: None
        })),
        "ok"
    );
    let reply = build_reply(&m, "abcd1234", None);
    let row = reply.members.iter().find(|r| r.id_full == two).unwrap();
    assert_eq!(row.status, "excluded");
    assert!(m.apply(&Act::Unexclude(two)).unwrap());
}

#[test]
fn a_waiting_node_is_admitted_at_once() {
    let mut m = demo(DemoRole::Root);
    let anna = id_of(&m, "anna-laptop");
    assert_eq!(code(m.apply(&Act::Admit(anna.clone()))), "ok");
    let reply = build_reply(&m, "abcd1234", None);
    let row = reply.members.iter().find(|r| r.id_full == anna).unwrap();
    assert_eq!(row.status, "member");
}

#[test]
fn a_plain_member_cannot_exclude_or_admit() {
    let mut m = demo(DemoRole::Member);
    let two = id_of(&m, "peer-two");
    let anna = id_of(&m, "anna-laptop");
    assert_eq!(
        code(m.apply(&Act::Exclude {
            id: two,
            reason: None
        })),
        "not_admin"
    );
    assert_eq!(code(m.apply(&Act::Admit(anna))), "not_admin");
}
