//! Tests of the member-list actions of `NodeCtl` (`control_members.rs`): the request parsing, the refusal codes
//! and the answer of a node that is not running.

use super::*;
use crate::config::config_file::Settings;
use crate::transport::net::contract::pid;
use crate::web_gui::NodeControl;

fn is_invalid(e: &MemberError) -> bool {
    matches!(e, MemberError::Action(ActionError::Invalid(_)))
}

#[test]
fn bad_hex_is_invalid() {
    // given: a request with an id that is not a node id
    let req = MemberActionReq::Block("not-an-id".to_string());
    // when / then: parsing refuses it as invalid
    assert!(is_invalid(&parse_action(req).unwrap_err()));
}

#[test]
fn an_empty_id_is_invalid() {
    assert!(is_invalid(
        &parse_action(MemberActionReq::Unblock(String::new())).unwrap_err()
    ));
}

#[test]
fn leave_needs_no_id() {
    assert_eq!(
        parse_action(MemberActionReq::Leave).unwrap(),
        MemberAction::Leave
    );
}

#[test]
fn exclude_keeps_the_reason() {
    let id = pid(2);
    let req = MemberActionReq::Exclude {
        id: id.to_string(),
        reason: Some("spam".to_string()),
    };
    assert_eq!(
        parse_action(req).unwrap(),
        MemberAction::Exclude(id, Some("spam".to_string()))
    );
}

#[test]
fn other_requests_map_to_their_actions() {
    let id = pid(3);
    let text = id.to_string();
    assert_eq!(
        parse_action(MemberActionReq::Unexclude(text.clone())).unwrap(),
        MemberAction::Unexclude(id)
    );
    assert_eq!(
        parse_action(MemberActionReq::Block(text.clone())).unwrap(),
        MemberAction::Block(id)
    );
    assert_eq!(
        parse_action(MemberActionReq::Unblock(text.clone())).unwrap(),
        MemberAction::Unblock(id)
    );
    assert_eq!(
        parse_action(MemberActionReq::Admit(text.clone())).unwrap(),
        MemberAction::Admit(id)
    );
    assert_eq!(
        parse_action(MemberActionReq::Unadmit(text)).unwrap(),
        MemberAction::Unadmit(id)
    );
    assert!(is_invalid(
        &parse_action(MemberActionReq::Admit("nope".to_string())).unwrap_err()
    ));
}

#[test]
fn refusals_map_to_codes() {
    let code_of = |r| match refusal_error(r) {
        MemberError::Refused { code, .. } => code,
        other => panic!("not a refusal: {other:?}"),
    };
    assert_eq!(code_of(ActionRefusal::SelfTarget), "self_target");
    assert_eq!(code_of(ActionRefusal::NotMember), "not_member");
    assert_eq!(code_of(ActionRefusal::Left), "left");
    assert_eq!(code_of(ActionRefusal::NotPending), "not_pending");
    assert_eq!(code_of(ActionRefusal::NotConfirmed), "not_confirmed");
}

#[test]
fn a_write_failure_is_internal() {
    let err = refusal_error(ActionRefusal::Write("disk".to_string()));
    assert!(matches!(err, MemberError::Action(ActionError::Internal(m)) if m == "disk"));
}

#[tokio::test]
async fn members_without_a_running_node_is_busy() {
    // given: a control with no node
    let ctl = NodeCtl::new(&Settings::default());
    // when / then: the list and an action both answer busy
    let list = ctl.members().await.unwrap_err();
    assert!(
        matches!(list, MemberError::Action(ActionError::Busy(_))),
        "{list:?}"
    );
    let act = ctl.member_action(MemberActionReq::Leave).await.unwrap_err();
    assert!(
        matches!(act, MemberError::Action(ActionError::Busy(_))),
        "{act:?}"
    );
}
