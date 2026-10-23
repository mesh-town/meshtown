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

mod parse_action_forget {
    use super::*;

    #[test]
    fn forget_request_becomes_forget_action() {
        let id = pid(4);
        let act = parse_action(MemberActionReq::Forget(id.to_string())).unwrap();
        assert_eq!(act, MemberAction::Forget(id));
    }

    #[test]
    fn unforget_request_becomes_unforget_action() {
        let id = pid(5);
        let act = parse_action(MemberActionReq::Unforget(id.to_string())).unwrap();
        assert_eq!(act, MemberAction::Unforget(id));
    }

    #[test]
    fn id_is_trimmed_like_for_other_actions() {
        let id = pid(6);
        let act = parse_action(MemberActionReq::Forget(format!("  {id} "))).unwrap();
        assert_eq!(act, MemberAction::Forget(id));
    }

    #[test]
    fn forget_with_bad_id_is_invalid() {
        let err = parse_action(MemberActionReq::Forget("not-an-id".to_string())).unwrap_err();
        assert!(is_invalid(&err), "{err:?}");
    }

    #[test]
    fn unforget_with_empty_id_is_invalid() {
        let err = parse_action(MemberActionReq::Unforget(String::new())).unwrap_err();
        assert!(is_invalid(&err), "{err:?}");
    }
}

mod refusal_error_forget {
    use super::*;

    #[test]
    fn online_is_refused_with_code_online() {
        let (code, message) = refused_parts(ActionRefusal::Online);
        assert_eq!(code, "online");
        assert!(!message.is_empty(), "the page shows the message");
    }

    #[test]
    fn not_forgotten_is_refused_with_code_not_forgotten() {
        let (code, message) = refused_parts(ActionRefusal::NotForgotten);
        assert_eq!(code, "not_forgotten");
        assert!(!message.is_empty(), "the page shows the message");
    }

    #[test]
    fn new_codes_differ_from_each_other_and_from_old_ones() {
        let codes: Vec<&str> = [
            ActionRefusal::SelfTarget,
            ActionRefusal::NotMember,
            ActionRefusal::Left,
            ActionRefusal::NotPending,
            ActionRefusal::NotConfirmed,
            ActionRefusal::Online,
            ActionRefusal::NotForgotten,
        ]
        .into_iter()
        .map(|r| refused_parts(r).0)
        .collect();
        let unique: std::collections::BTreeSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len(), "codes: {codes:?}");
    }
}

mod forget_through_the_node {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn forget_of_own_node_is_refused_self_target() {
        let fx = setup(Opts::default()).await;
        let err = fx.when_action(MemberActionReq::Forget(fx.node.id.to_string())).await;
        assert_eq!(refusal_code(&err), "self_target");
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn forget_of_unknown_node_is_refused_not_member() {
        let fx = setup(Opts::default()).await;
        let err = fx.when_action(MemberActionReq::Forget(pid(9).to_string())).await;
        assert_eq!(refusal_code(&err), "not_member");
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unforget_without_own_vote_is_refused_not_forgotten() {
        let fx = setup(Opts::default()).await;
        let err = fx.when_action(MemberActionReq::Unforget(pid(9).to_string())).await;
        assert_eq!(refusal_code(&err), "not_forgotten");
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn forget_with_bad_id_is_invalid_before_the_node_is_asked() {
        let fx = setup(Opts::default()).await;
        let err = fx.when_action(MemberActionReq::Forget("zz".to_string())).await;
        assert!(is_invalid(&err), "{err:?}");
        fx.finish().await;
    }
}

mod seen_ages {
    use super::*;

    const NOW: u64 = 2_000_000;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cached_node_age_is_now_minus_seen_time() {
        let fx = setup(Opts::with_cached(pid(7), NOW - 90)).await;
        let ages = fx.when_seen_ages(NOW).await;
        assert_eq!(ages.get(&pid(7)), Some(&90));
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn node_without_any_sign_is_absent() {
        let fx = setup(Opts::with_cached(pid(7), NOW - 90)).await;
        let ages = fx.when_seen_ages(NOW).await;
        assert!(!ages.contains_key(&pid(8)), "{ages:?}");
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn seen_time_in_the_future_gives_zero_age() {
        let fx = setup(Opts::with_cached(pid(7), NOW + 500)).await;
        let ages = fx.when_seen_ages(NOW).await;
        assert_eq!(ages.get(&pid(7)), Some(&0));
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn node_without_cache_entries_has_no_cached_ages() {
        let fx = setup(Opts::default()).await;
        let ages = fx.when_seen_ages(NOW).await;
        assert!(!ages.contains_key(&pid(7)), "{ages:?}");
        fx.finish().await;
    }
}

mod members_reply_last_seen {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn row_of_cached_node_has_its_age() {
        let seen = crate::transport::net_mainline::unix_now() - 120;
        let fx = setup(Opts::with_cached(pid(7), seen)).await;
        fx.given_row_for(pid(7)).await;
        let reply = fx.when_reply().await;
        let age = row_of(&reply, pid(7)).last_seen_sec.expect("age from the cache");
        assert!((120..=180).contains(&age), "age {age}");
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn row_without_any_sign_has_null_age() {
        let fx = setup(Opts::default()).await;
        fx.given_row_for(pid(7)).await;
        let reply = fx.when_reply().await;
        assert_eq!(row_of(&reply, pid(7)).last_seen_sec, None);
        fx.finish().await;
    }
}

#[derive(Default)]
struct Opts {
    cached: Vec<(PeerId, u64)>,
}

impl Opts {
    fn with_cached(id: PeerId, seen_unix: u64) -> Self {
        Opts {
            cached: vec![(id, seen_unix)],
        }
    }
}

struct Fixture {
    node: crate::node::RunningNode,
    ctl: Arc<NodeCtl>,
}

/// A loopback node whose address cache holds `opts.cached`, attached to a control.
async fn setup(opts: Opts) -> Fixture {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let mut cfg = crate::node::NodeConfig::for_loopback_test(key);
    cfg.socks.enabled = false;
    let mut node = crate::node::start_node(cfg).await.unwrap();
    node.cached_peers = Arc::new(opts.cached.into_iter().collect());
    let ctl = NodeCtl::new(&Settings::default());
    ctl.attach(&node);
    wait_until_actions_are_served(&ctl).await;
    Fixture { node, ctl }
}

/// The control serves actions once its first snapshot is collected.
async fn wait_until_actions_are_served(ctl: &NodeCtl) {
    for _ in 0..200 {
        if ctl.running_node_for_action().is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the control did not start serving actions in 10 s");
}

impl Fixture {
    async fn when_action(&self, req: MemberActionReq) -> MemberError {
        self.ctl
            .member_action(req)
            .await
            .expect_err("the action must be refused")
    }

    async fn when_seen_ages(&self, now_unix: u64) -> std::collections::BTreeMap<PeerId, u64> {
        super::super::members_seen::seen_ages(&self.node, now_unix).await
    }

    async fn when_reply(&self) -> MembersReply {
        reply_of(&self.node, None).await.expect("the list of a running node")
    }

    /// Makes the node a row of the list without making it a member: the own block list names it.
    async fn given_row_for(&self, id: PeerId) {
        apply(&self.node.access, MemberAction::Block(id))
            .await
            .expect("blocking a node is always possible");
    }

    async fn finish(self) {
        self.node.shutdown().await;
    }
}

fn refused_parts(r: ActionRefusal) -> (&'static str, String) {
    match refusal_error(r) {
        MemberError::Refused { code, message } => (code, message),
        other => panic!("not a refusal: {other:?}"),
    }
}

fn refusal_code(e: &MemberError) -> &'static str {
    match e {
        MemberError::Refused { code, .. } => code,
        other => panic!("not a refusal: {other:?}"),
    }
}

fn row_of(reply: &MembersReply, id: PeerId) -> &crate::web_gui::MemberRow {
    let full = id.to_string();
    reply
        .members
        .iter()
        .find(|r| r.id_full == full)
        .unwrap_or_else(|| panic!("no row for {full}"))
}
