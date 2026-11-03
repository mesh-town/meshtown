use super::*;
use crate::invite::{InviteHead, InviteSecret};
use crate::members::network_key::{managed_key_with_label, parse_user_input};
use crate::transport::net::{NodeSecret, PeerAddr};
use crate::web_gui::test_host::{config_with_key, slot_handle};
use crate::web_gui::{
    ActionError, Counts, LogPage, MemberError, MemberFuture, MemberRow, NodeControl, NodeSnapshot,
    RefusalInfo, SaveKey,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const TICK: Duration = Duration::from_millis(50);

// ---------- standing_of ----------

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
        invited_as: None,
    }
}

fn reply(me_excluded: bool, members: Vec<MemberRow>) -> MembersReply {
    MembersReply {
        me: String::new(),
        me_excluded,
        me_pending: false,
        admins: vec![],
        me_admin: false,
        root: None,
        me_admission: None,
        counts: Counts {
            members: 0,
            excluded: 0,
            unlisted: 0,
            pending: 0,
        },
        members,
        changed: None,
    }
}

fn unlisted_reply() -> MembersReply {
    reply(
        false,
        vec![
            row(true, "unlisted", None),
            row(false, "member", Some(("them", "unlisted"))),
        ],
    )
}

fn member_reply() -> MembersReply {
    reply(false, vec![row(true, "member", None)])
}

#[test]
fn member_follows_the_self_row() {
    let s = standing_of(&member_reply());
    assert!(s.member && !s.unlisted);
}

#[test]
fn unlisted_needs_a_refusal_from_them() {
    assert!(standing_of(&unlisted_reply()).unlisted);
}

#[test]
fn a_refusal_on_this_side_is_not_unlisted() {
    let r = reply(
        false,
        vec![
            row(true, "unlisted", None),
            row(false, "member", Some(("me", "unlisted"))),
        ],
    );
    assert!(!standing_of(&r).unlisted);
}

#[test]
fn a_member_with_an_old_refusal_is_not_unlisted() {
    let r = reply(
        false,
        vec![
            row(true, "member", None),
            row(false, "member", Some(("them", "unlisted"))),
        ],
    );
    let s = standing_of(&r);
    assert!(s.member && !s.unlisted);
}

#[test]
fn a_them_refusal_for_another_reason_is_not_unlisted() {
    let r = reply(
        false,
        vec![
            row(true, "unlisted", None),
            row(false, "member", Some(("them", "excluded"))),
        ],
    );
    assert!(!standing_of(&r).unlisted);
}

// ---------- invite_need ----------

fn st(member: bool, unlisted: bool, excluded: bool) -> Option<Standing> {
    Some(Standing {
        member,
        unlisted,
        excluded,
    })
}

#[test]
fn a_member_keeps_the_invite() {
    assert_eq!(
        invite_need(st(true, true, false), true, 10, 100),
        Need::Member
    );
}

#[test]
fn a_member_past_the_deadline_is_still_a_member() {
    assert_eq!(
        invite_need(st(true, false, false), true, 200, 100),
        Need::Member
    );
}

#[test]
fn waits_before_the_node_has_published_verdicts() {
    assert_eq!(invite_need(None, true, 10, 100), Need::Wait);
}

#[test]
fn waits_while_no_neighbour_has_turned_it_away() {
    assert_eq!(
        invite_need(st(false, false, false), true, 10, 100),
        Need::Wait
    );
}

#[test]
fn spends_once_a_neighbour_turns_it_away_as_unlisted() {
    assert_eq!(
        invite_need(st(false, true, false), true, 10, 100),
        Need::Spend
    );
}

#[test]
fn spends_one_second_before_the_deadline() {
    assert_eq!(
        invite_need(st(false, true, false), true, 99, 100),
        Need::Spend
    );
}

#[test]
fn spends_when_the_network_excluded_this_computer() {
    assert_eq!(
        invite_need(st(false, false, true), true, 10, 100),
        Need::Spend
    );
}

#[test]
fn expires_at_the_invite_deadline_without_spending() {
    assert_eq!(
        invite_need(st(false, true, false), true, 100, 100),
        Need::Expired
    );
}

#[test]
fn is_elsewhere_when_the_node_runs_another_network() {
    assert_eq!(
        invite_need(st(false, true, false), false, 10, 100),
        Need::Elsewhere
    );
}

// ---------- watch_admission ----------

/// Answers `members()` from a queue (the last answer repeats) and records what `join_by_invite` got.
struct QueueNode {
    answers: Mutex<VecDeque<Option<MembersReply>>>,
    joins: Mutex<Vec<(InviteHead, Vec<PeerAddr>)>>,
    join_result: Result<(), ActionError>,
}

impl QueueNode {
    fn new(
        answers: Vec<Option<MembersReply>>,
        join_result: Result<(), ActionError>,
    ) -> Arc<QueueNode> {
        Arc::new(QueueNode {
            answers: Mutex::new(answers.into()),
            joins: Mutex::new(vec![]),
            join_result,
        })
    }

    fn join_count(&self) -> usize {
        self.joins.lock().unwrap().len()
    }
}

impl NodeControl for QueueNode {
    fn snapshot(&self) -> NodeSnapshot {
        NodeSnapshot::default()
    }
    fn log_after(&self, _after: u64, _limit: usize, _net: Option<&str>) -> LogPage {
        LogPage::default()
    }
    fn find_exit(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_domain(&self, _domain: &str) -> Result<(), ActionError> {
        Ok(())
    }
    fn check_all(&self) -> Result<u64, ActionError> {
        Ok(0)
    }
    fn add_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn remove_domain(&self, _domain: &str) -> Result<String, ActionError> {
        Ok(String::new())
    }
    fn add_peer_address(&self, _line: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_config(&self, _s: &crate::config::config_file::Settings, _p: &[&'static str]) {}
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}
    fn join_by_invite(
        &self,
        head: InviteHead,
        peers: Vec<PeerAddr>,
        _save: SaveKey,
    ) -> Result<(), ActionError> {
        self.joins.lock().unwrap().push((head, peers));
        self.join_result.clone()
    }
    fn members(&self) -> MemberFuture {
        let mut q = self.answers.lock().unwrap();
        let next = if q.len() > 1 {
            q.pop_front().unwrap()
        } else {
            q.front().cloned().unwrap_or(None)
        };
        let out = next.ok_or(MemberError::Action(ActionError::Busy(
            "starting".to_string(),
        )));
        Box::pin(async move { out })
    }
}

fn key_text() -> String {
    managed_key_with_label(
        None,
        &[9; 32],
        &NodeSecret::from_bytes(&[1u8; 32]).peer_id(),
    )
}

fn invite_made(made_unix: u64) -> ParsedInvite {
    let network_id = parse_user_input(&key_text())
        .unwrap()
        .key
        .subkeys()
        .network_id;
    ParsedInvite {
        head: InviteHead {
            secret: InviteSecret::from_bytes([0xC7; 32]),
            network_id,
            label: None,
            made_unix,
            root: NodeSecret::from_bytes(&[1u8; 32]).peer_id(),
        },
        peers: vec![],
        warnings: vec![],
    }
}

/// An invite with a long way to the deadline.
fn fresh() -> ParsedInvite {
    invite_made(now_unix())
}

async fn watch(node: &Arc<QueueNode>, key: Option<&str>, inv: ParsedInvite) {
    let slot = slot_handle("main", node.clone(), config_with_key(key));
    let task = watch_admission(slot, inv, TICK);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("the watch must end");
}

#[tokio::test]
async fn restart_of_a_member_never_spends_the_invite() {
    let node = QueueNode::new(vec![Some(member_reply())], Ok(()));
    watch(&node, Some(&key_text()), fresh()).await;
    assert_eq!(node.join_count(), 0);
}

#[tokio::test]
async fn pending_then_member_never_spends_the_invite() {
    let node = QueueNode::new(vec![None, None, Some(member_reply())], Ok(()));
    watch(&node, Some(&key_text()), fresh()).await;
    assert_eq!(node.join_count(), 0);
}

#[tokio::test]
async fn an_unlisted_node_hands_the_invite_to_the_node_once() {
    let node = QueueNode::new(
        vec![
            Some(reply(false, vec![row(true, "unlisted", None)])),
            Some(unlisted_reply()),
        ],
        Ok(()),
    );
    let inv = fresh();
    watch(&node, Some(&key_text()), inv.clone()).await;
    let joins = node.joins.lock().unwrap();
    assert_eq!(joins.len(), 1);
    assert_eq!(joins[0].0.secret, inv.head.secret);
    assert_eq!(joins[0].1, inv.peers);
}

#[tokio::test]
async fn an_excluded_node_hands_the_invite_to_the_node_once() {
    let node = QueueNode::new(
        vec![Some(reply(true, vec![row(true, "excluded", None)]))],
        Ok(()),
    );
    let inv = fresh();
    watch(&node, Some(&key_text()), inv.clone()).await;
    let joins = node.joins.lock().unwrap();
    assert_eq!(joins.len(), 1);
    assert_eq!(joins[0].0.secret, inv.head.secret);
    assert_eq!(joins[0].1, inv.peers);
}

#[tokio::test]
async fn a_refused_join_ends_the_watch_quietly() {
    let refused = Err(ActionError::Busy("no".to_string()));
    let node = QueueNode::new(vec![Some(unlisted_reply())], refused);
    watch(&node, Some(&key_text()), fresh()).await;
    assert_eq!(node.join_count(), 1);
}

#[tokio::test]
async fn no_neighbours_until_the_deadline_spends_nothing() {
    let quiet = reply(false, vec![row(true, "unlisted", None)]);
    let node = QueueNode::new(vec![Some(quiet)], Ok(()));
    let inv = invite_made(now_unix() - INVITE_TTL_SECS + 1);
    watch(&node, Some(&key_text()), inv).await;
    assert_eq!(node.join_count(), 0);
}

#[tokio::test]
async fn a_switched_network_spends_nothing() {
    let node = QueueNode::new(vec![Some(unlisted_reply())], Ok(()));
    let other = managed_key_with_label(
        None,
        &[8; 32],
        &NodeSecret::from_bytes(&[1u8; 32]).peer_id(),
    );
    watch(&node, Some(&other), fresh()).await;
    assert_eq!(node.join_count(), 0);
}

#[tokio::test]
async fn a_slot_without_a_key_spends_nothing() {
    let node = QueueNode::new(vec![Some(unlisted_reply())], Ok(()));
    watch(&node, None, fresh()).await;
    assert_eq!(node.join_count(), 0);
}
