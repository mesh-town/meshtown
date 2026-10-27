//! `PUT /api/config` with a `ports.opened` key: the list is applied to the node even when it equals the
//! saved one, so an entry whose bind failed is retried. The harness is from `mod_tests.rs` via `use super::*;`.

use super::*;
use crate::members::membership::Members;
use crate::ports_sharing::open::{OpenState, Opener};
use crate::transport::net::facade::test_support::secret;
use crate::transport::net::{MemNet, NetStackBuilder, NodeSecret};

fn opens(locals: &[u16]) -> Value {
    let node = NodeSecret::from_bytes(&[7u8; 32]).peer_id().to_string();
    let list: Vec<Value> = locals
        .iter()
        .map(|local| serde_json::json!({"node": node, "port": 80, "local": local}))
        .collect();
    serde_json::json!({"ports": {"opened": list}})
}

/// Sends `config` at the current revision; returns the status and the `changed` list of the answer.
fn put(state: &mut ConfigState, node: &dyn NodeControl, config: Value) -> (StatusCode, Value) {
    let body = put_body(state.file_report.rev, config);
    let (status, v) = body_json(state.put(&body, node));
    (status, v["changed"].clone())
}

fn applied_of(node: &StubNode) -> Vec<Vec<String>> {
    node.applied.lock().unwrap().clone()
}

#[test]
fn put_same_opened_reapplies() {
    // given: the list is saved and applied once
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    put(&mut state, &node, opens(&[9100]));
    // when: the same list again
    let (status, changed) = put(&mut state, &node, opens(&[9100]));
    // then: applied again, but the answer stays honest
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed, serde_json::json!([]));
    let want = vec!["ports.opened".to_string()];
    assert_eq!(applied_of(&node), vec![want.clone(), want]);
    cleanup(&dir);
}

#[test]
fn put_without_opened_applies_nothing() {
    // given
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = StubNode::new();
    // when: a body without `ports.opened` changes nothing
    let (status, changed) = put(&mut state, &node, serde_json::json!({}));
    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed, serde_json::json!([]));
    assert!(applied_of(&node).is_empty());
    cleanup(&dir);
}

#[test]
fn put_opened_cli_override_not_applied() {
    // given: the path is fixed by the command line
    let dir = temp_dir();
    let mut state = new_state(&dir);
    state.cli_override_paths = vec!["ports.opened"];
    let node = StubNode::new();
    // when
    put(&mut state, &node, opens(&[9100]));
    put(&mut state, &node, opens(&[9100]));
    // then
    assert!(applied_of(&node).is_empty());
    cleanup(&dir);
}

/// A node whose `ports.opened` goes to a real `Opener`, as in the running node.
struct OpenerNode {
    opener: Opener,
    rest: StubNode,
}

impl NodeControl for OpenerNode {
    fn snapshot(&self) -> NodeSnapshot {
        self.rest.snapshot()
    }
    fn log_after(&self, after: u64, limit: usize, net: Option<&str>) -> LogPage {
        self.rest.log_after(after, limit, net)
    }
    fn find_exit(&self, d: &str) -> Result<(), ActionError> {
        self.rest.find_exit(d)
    }
    fn check_domain(&self, d: &str) -> Result<(), ActionError> {
        self.rest.check_domain(d)
    }
    fn check_all(&self) -> Result<u64, ActionError> {
        self.rest.check_all()
    }
    fn add_domain(&self, d: &str) -> Result<String, ActionError> {
        self.rest.add_domain(d)
    }
    fn remove_domain(&self, d: &str) -> Result<String, ActionError> {
        self.rest.remove_domain(d)
    }
    fn add_peer_address(&self, line: &str) -> Result<(), String> {
        self.rest.add_peer_address(line)
    }
    fn apply_config(&self, settings: &Settings, paths: &[&'static str]) {
        if paths.contains(&"ports.opened") {
            self.opener
                .reconcile_listeners(settings.net.ports_opened.clone());
        }
    }
    fn switch_network(&self, _key: &str) {}
    fn stop(&self) {}
}

fn opener_node() -> OpenerNode {
    let opener = block_on(async {
        let net = MemNet::new();
        let stack = NetStackBuilder::new(secret(4))
            .transport(net.transport(&secret(4)))
            .build()
            .expect("single-node stack");
        Opener::new(stack, Members::default())
    });
    OpenerNode {
        opener,
        rest: StubNode::new(),
    }
}

#[test]
fn put_same_opened_retries_a_failed_bind() {
    // given: the local port is taken, so the first PUT ends in `bind_failed`
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let local = busy.local_addr().unwrap().port();
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let node = opener_node();
    put(&mut state, &node, opens(&[local]));
    let states = |n: &OpenerNode| n.opener.bind_states();
    assert!(matches!(states(&node)[0].1, OpenState::BindFailed(_)));
    // when: the port is freed and the same list is sent again
    drop(busy);
    let (_, changed) = put(&mut state, &node, opens(&[local]));
    // then
    assert_eq!(changed, serde_json::json!([]));
    assert_eq!(states(&node)[0].1, OpenState::Listening);
    cleanup(&dir);
}
