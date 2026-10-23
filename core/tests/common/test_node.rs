//! A fake node `TestNode` for the `tests/web_listener_*.rs` scenarios: a call log under
//! `Mutex<Vec<String>>`, the status and the invite-join state under `Mutex`. Moved out of
//! `web_listener.rs` (the file hit the 400-line ceiling); included from there
//! (`#[path = "test_node.rs"] mod test_node;`) and exposed as `common::TestNode`.
//! What it doesn't do: no real actions — each method writes its name to the log and answers with success.
#![allow(dead_code)] // each test crate takes its own subset

use std::sync::Mutex;

use meshtown_core::config::config_file;
use meshtown_core::invite::InviteHead;
use meshtown_core::transport::net::PeerAddr;
use meshtown_core::web_gui::{
    ActionError, InviteJoinView, LogLine, LogPage, LogSummary, NodeControl, NodeRuntime,
    NodeSnapshot, NodeStatus, SaveKey, TransportInfo,
};

/// A fake node for tests: a call log under `Mutex<Vec<String>>`, the status under `Mutex`.
pub(crate) struct TestNode {
    call_log: Mutex<Vec<String>>,
    status: Mutex<NodeStatus>,
    invite_join: Mutex<Option<InviteJoinView>>,
    /// The key that `join_by_invite` passes to `save`, the way a node does when it receives a network key.
    key_for_save: Mutex<Option<String>>,
}

impl TestNode {
    pub(crate) fn new() -> Self {
        TestNode {
            call_log: Mutex::new(Vec::new()),
            status: Mutex::new(NodeStatus::Online),
            invite_join: Mutex::new(None),
            key_for_save: Mutex::new(None),
        }
    }

    fn push_call(&self, name: &str) {
        self.call_log.lock().unwrap().push(name.to_string());
    }

    /// `true` if `name` was among the calls (used for "TestNode got switch/stop").
    pub(crate) fn got_call(&self, name: &str) -> bool {
        self.call_log.lock().unwrap().iter().any(|c| c == name)
    }

    /// Sets what the node will show as the state of joining by invite (`node.invite_join`).
    pub(crate) fn set_invite_join(&self, view: Option<InviteJoinView>) {
        *self.invite_join.lock().unwrap() = view;
    }

    /// After this `join_by_invite` calls the passed `save` with `key_text` (by default it doesn't).
    pub(crate) fn save_key_on_join(&self, key_text: &str) {
        *self.key_for_save.lock().unwrap() = Some(key_text.to_string());
    }
}

/// The last 6 lines of the internal 8 (`docs/WEB-API.md` §1.3 gives exactly 6 in `log.tail`). `TestNode` builds
/// 8, gives out the last 6, rounding `t` to one decimal place.
fn log_tail_6() -> Vec<LogLine> {
    let all: Vec<LogLine> = (1u64..=8)
        .map(|i| {
            let t = (i as f64) * 1.234;
            LogLine {
                seq: i,
                t: (t * 10.0).round() / 10.0,
                tag: "node".to_string(),
                text: format!("event {i}"),
                ..Default::default()
            }
        })
        .collect();
    all[2..].to_vec()
}

impl NodeControl for TestNode {
    fn snapshot(&self) -> NodeSnapshot {
        let status = *self.status.lock().unwrap();
        NodeSnapshot {
            boot_id: "a1b2c3d4".to_string(),
            now: 12.345,
            node: NodeRuntime {
                status,
                id: "a1b2c3d4e8".to_string(),
                id_full: "a1b2c3d4e8f01234567890abcdef0123".to_string(),
                uptime_sec: 42,
                rss_kib: 12_345,
                rss_peak_kib: 12_345,
            },
            transport: TransportInfo {
                self_addresses: vec!["254b24c7d9@192.168.1.14:47821".to_string()],
                ..Default::default()
            },
            log: LogSummary {
                tail: log_tail_6(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn log_after(&self, after: u64, limit: usize, _net: Option<&str>) -> LogPage {
        // Defensive clamp: limit=0 must give 1 line,
        // not depending on whether the caller clamps before the call.
        let limit = limit.max(1);
        let all: Vec<LogLine> = (11u64..=20)
            .map(|seq| LogLine {
                seq,
                t: (seq as f64) / 10.0,
                tag: "node".to_string(),
                text: format!("log line {seq}"),
                ..Default::default()
            })
            .collect();
        let lines: Vec<LogLine> = all
            .into_iter()
            .filter(|l| l.seq > after)
            .take(limit)
            .collect();
        LogPage {
            boot_id: "a1b2c3d4".to_string(),
            first_seq: 11,
            last_seq: 20,
            lines,
        }
    }

    fn find_exit(&self, _domain: &str) -> Result<(), ActionError> {
        self.push_call("find_exit");
        Ok(())
    }

    fn check_domain(&self, domain: &str) -> Result<(), ActionError> {
        self.push_call("check_domain");
        if domain == "busy.example" {
            Err(ActionError::Busy(
                "this domain check is already running".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn check_all(&self) -> Result<u64, ActionError> {
        self.push_call("check_all");
        Ok(0)
    }

    fn add_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.push_call("add_domain");
        Ok(domain.to_string())
    }

    fn remove_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.push_call("remove_domain");
        Ok(domain.to_string())
    }

    fn add_peer_address(&self, _line: &str) -> Result<(), String> {
        self.push_call("add_peer_address");
        Ok(())
    }

    fn apply_config(&self, _settings: &config_file::Settings, _paths: &[&'static str]) {
        self.push_call("apply_config");
    }

    fn switch_network(&self, _key: &str) {
        self.push_call("switch");
    }

    fn stop(&self) {
        *self.status.lock().unwrap() = NodeStatus::Stopping;
        self.push_call("stop");
    }

    fn restart(&self) -> Result<(), ActionError> {
        self.push_call("restart");
        Ok(())
    }

    fn join_by_invite(
        &self,
        _head: InviteHead,
        _peers: Vec<PeerAddr>,
        save: SaveKey,
    ) -> Result<(), ActionError> {
        self.push_call("join_by_invite");
        if let Some(text) = self.key_for_save.lock().unwrap().clone() {
            let _ = save(&text);
        }
        Ok(())
    }

    fn invite_join(&self) -> Option<InviteJoinView> {
        self.invite_join.lock().unwrap().clone()
    }
}
