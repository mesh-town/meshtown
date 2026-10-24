use meshtown_core::transport::net::NodeSecret;
use meshtown_core::web_gui::{MemberActionReq, MemberError};

use super::*;
use crate::members_reply::build_reply;
use crate::node::{DemoNetworkOptions, DemoPeer};

fn peer(seed: u8, name: &str) -> DemoPeer {
    let id = NodeSecret::from_bytes(&[seed; 32]).peer_id();
    DemoPeer {
        id: id.to_string()[..8].to_string(),
        id_full: id,
        name: Some(name.to_string()),
        path: None,
        online: true,
    }
}

fn demo() -> DemoMembers {
    let opts = DemoNetworkOptions {
        offline_peers: 0,
        dht_joined: false,
        dht_nodes: 0,
        members_second_vote: false,
        me_excluded: false,
        members_pending: false,
        me_pending: false,
        members_forget: true,
        me_removed: false,
    };
    DemoMembers::new(&[peer(2, "peer-one"), peer(3, "peer-two")], &opts)
}

fn at(m: &DemoMembers, name: &str) -> usize {
    m.entries
        .iter()
        .position(|e| e.name.as_deref() == Some(name))
        .unwrap()
}

fn code(err: MemberError) -> &'static str {
    match err {
        MemberError::Refused { code, .. } => code,
        _ => "other",
    }
}

#[test]
fn forget_online_refused() {
    let mut m = demo();
    let i = at(&m, "peer-one");
    assert_eq!(code(m.forget(i).unwrap_err()), "online");
}

#[test]
fn need_two_waits() {
    let mut m = demo();
    let i = at(&m, "old-desktop");
    assert!(m.forget(i).unwrap());
    assert!(matches!(m.entries[i].standing, Standing::Member));
    assert_eq!(m.entries[i].forget.voters, vec![ME.to_string()]);
    assert!(!m.forget(i).unwrap());
}

#[test]
fn second_vote_forgets() {
    let mut m = demo();
    let i = at(&m, "lost-phone");
    assert!(m.forget(i).unwrap());
    assert!(matches!(m.entries[i].standing, Standing::Forgotten));
    assert!(m.entries[i].forget.at.is_some());
}

#[test]
fn unforget_returns_member() {
    let mut m = demo();
    let i = at(&m, "old-tablet");
    assert!(m.unforget(i).unwrap());
    assert!(matches!(m.entries[i].standing, Standing::Member));
    assert_eq!(m.entries[i].forget.at, None);
}

#[test]
fn unforget_without_vote_refused() {
    let mut m = demo();
    let i = at(&m, "old-desktop");
    assert_eq!(code(m.unforget(i).unwrap_err()), "not_forgotten");
}

#[test]
fn reply_marks_mine_removed_by_me() {
    let mut m = demo();
    let i = at(&m, "old-desktop");
    m.apply(&MemberActionReq::Forget(m.entries[i].id_full.clone()))
        .unwrap();
    let reply = build_reply(&m, "1234abcd", None);
    let row = reply
        .members
        .iter()
        .find(|r| r.name.as_deref() == Some("old-desktop"))
        .unwrap();
    assert_eq!(row.denied_by_me, Some("removed_by_me"));
    assert!(row.forget.as_ref().unwrap().mine);
    assert_eq!(row.last_seen_sec, Some(777_600));
    assert_eq!(reply.counts.forgotten, 1);
}
