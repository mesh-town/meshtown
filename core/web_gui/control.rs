//! The node trait for the listener (`NodeControl`) and action/start errors (`ActionError`,
//! `WebStartError`).

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;

use super::{LogPage, MembersReply, NetRoles, NodeSnapshot};
use crate::invite::Dealt;
use crate::node::output::NET_NAME;
use crate::transport::net::NodeSecret;

/// Writes the network key to the settings file (and the new node key, if the join changed it): the page hands it
/// to the node, the node calls it when the key is received.
pub type SaveKey = Box<dyn FnOnce(&str, Option<&NodeSecret>) -> Result<(), String> + Send>;

/// Invite-join state for `/api/state` (`node.invite_join`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteJoinState {
    Waiting,
    Refused,
}

/// Waiting for or refusal of joining by invite; disappears on cancel, join and stop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InviteJoinView {
    pub state: InviteJoinState,
    pub message: String,
    pub label: Option<String>,
    pub expires_unix: u64,
}

/// An invite that is still being completed: the issue record in the network document is written after the string is built,
/// and the string is given out only when the write succeeded.
pub struct PendingInvite(Pin<Box<dyn Future<Output = anyhow::Result<Dealt>> + Send>>);

impl PendingInvite {
    /// An invite with nothing left to complete (stubs): no holders.
    pub fn ready(text: String) -> Self {
        PendingInvite(Box::pin(std::future::ready(Ok(Dealt::without_holders(
            text,
        )))))
    }

    /// An invite ready after `fut` runs.
    pub fn pending(fut: impl Future<Output = anyhow::Result<Dealt>> + Send + 'static) -> Self {
        PendingInvite(Box::pin(fut))
    }

    /// Waits for the issue record and returns the string; an error is `Internal`.
    pub async fn finish(self) -> Result<String, ActionError> {
        self.finish_dealt().await.map(|d| d.text)
    }

    /// The same, but with holders: the string and the members that kept the record.
    pub async fn finish_dealt(self) -> Result<Dealt, ActionError> {
        self.0
            .await
            .map_err(|e| ActionError::Internal(format!("invite could not be built: {e:#}")))
    }
}

pub trait NodeControl: Send + Sync {
    fn snapshot(&self) -> NodeSnapshot;
    fn log_after(&self, after: u64, limit: usize, net: Option<&str>) -> LogPage;
    fn find_exit(&self, domain: &str) -> Result<(), ActionError>;
    fn check_domain(&self, domain: &str) -> Result<(), ActionError>;
    fn check_all(&self) -> Result<u64, ActionError>;
    fn add_domain(&self, domain: &str) -> Result<String, ActionError>;
    fn remove_domain(&self, domain: &str) -> Result<String, ActionError>;
    fn add_peer_address(&self, line: &str) -> Result<(), String>;
    fn apply_config(&self, settings: &crate::config::config_file::Settings, paths: &[&'static str]);
    fn switch_network(&self, key: &str);
    fn stop(&self);
    fn invite(&self) -> Result<String, ActionError> {
        Err(ActionError::NotFound("action not implemented".to_string()))
    }
    /// An invite with a deferred issue record; by default a ready string from `invite`, which ignores the note
    /// `for_whom` (who the invite is for, shown to the members).
    fn invite_pending(&self, _for_whom: Option<&str>) -> Result<PendingInvite, ActionError> {
        self.invite().map(PendingInvite::ready)
    }
    fn restart(&self) -> Result<(), ActionError> {
        Err(ActionError::NotFound("action not implemented".to_string()))
    }
    fn join_with_peers(&self, key: &str, peers: Vec<crate::transport::net::PeerAddr>) {
        let _ = peers;
        self.switch_network(key)
    }
    /// Join by invite: waiting for members happens on the node, the key is written by `save` once received.
    fn join_by_invite(
        &self,
        head: crate::invite::InviteHead,
        peers: Vec<crate::transport::net::PeerAddr>,
        save: SaveKey,
    ) -> Result<(), ActionError> {
        let _ = (head, peers, save);
        Err(ActionError::NotFound("action not implemented".to_string()))
    }
    /// What is happening with the invite join now; `None` — no waiting.
    fn invite_join(&self) -> Option<InviteJoinView> {
        None
    }
    /// The member list for `GET /api/members`.
    fn members(&self) -> MemberFuture {
        not_implemented()
    }
    /// What this node may do in the network; `None` — the node has published nothing yet.
    fn net_roles(&self) -> Option<NetRoles> {
        None
    }
    /// A member action (exclude, unexclude, block, unblock, leave); the reply is the list after it.
    fn member_action(&self, act: MemberActionReq) -> MemberFuture {
        let _ = act;
        not_implemented()
    }
}

/// The reply of an asynchronous member-list call.
pub type MemberFuture = Pin<Box<dyn Future<Output = Result<MembersReply, MemberError>> + Send>>;

fn not_implemented() -> MemberFuture {
    Box::pin(async {
        Err(MemberError::Action(ActionError::NotFound(
            "action not implemented".to_string(),
        )))
    })
}

/// A member-list failure: a common action error, or a refusal with its own code (HTTP 409).
#[derive(Debug)]
pub enum MemberError {
    Action(ActionError),
    Refused { code: &'static str, message: String },
}

/// A member action asked by the page; ids are 64-hex node ids as sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberActionReq {
    Exclude {
        id: String,
        reason: Option<String>,
    },
    Unexclude(String),
    Admit(String),
    Block(String),
    Unblock(String),
    /// Make the node an administrator of a managed network.
    Grant(String),
    /// Take the administrator role away.
    Revoke(String),
    /// Delete the network (administrators only).
    Close,
    Leave,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActionError {
    Busy(String),
    /// The node has no network yet, so the action has nothing to act on.
    NoNetwork(String),
    NotFound(String),
    Invalid(String),
    /// A refusal of a member action with its own code (answered 409 `{error: code, message}`).
    Denied {
        code: &'static str,
        message: String,
    },
    Internal(String),
}

#[derive(Debug)]
pub enum WebStartError {
    Disabled,
    Bind {
        port: u16,
        reason: String,
        kind: std::io::ErrorKind,
    },
    Random(String),
}

impl std::fmt::Display for WebStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebStartError::Disabled => write!(f, "the page is turned off in {NET_NAME} settings"),
            WebStartError::Bind { port, reason, kind } => {
                let head = port_phrase(*port, *kind);
                write!(f, "the page didn't start: {head} ({reason})")
            }
            WebStartError::Random(reason) => write!(
                f,
                "the page didn't start: no random token obtained ({reason})"
            ),
        }
    }
}

/// The page port for humans: by error kind — taken by another program or simply didn't open.
fn port_phrase(port: u16, kind: std::io::ErrorKind) -> String {
    if kind == std::io::ErrorKind::AddrInUse {
        return format!("port {port} is in use by another program");
    }
    format!("port {port} didn't open")
}

/// A hint about another port: `--web-port` with the next port (at 65535, the previous one).
fn other_port_hint(port: u16) -> String {
    let other = if port == u16::MAX { port - 1 } else { port + 1 };
    format!(
        "{} run --web-port {other}",
        crate::cli::program_name::program_name()
    )
}

impl WebStartError {
    /// A log line for when a node with a key goes on without the page; `Disabled` — no line.
    pub fn continue_without_page_log_line(&self) -> Option<String> {
        match self {
            WebStartError::Disabled => None,
            WebStartError::Bind { port, reason, kind } => Some(bind_line(*port, *kind, reason)),
            WebStartError::Random(reason) => Some(format!(
                "the page didn't start: no random token obtained ({reason}) — {NET_NAME} is running without the page"
            )),
        }
    }

    /// Advice for a start error without a key: depends on the cause, mentions the port only if the cause is the port.
    pub fn start_failure_advice(&self) -> Option<String> {
        match self {
            WebStartError::Disabled => Some(
                "The page is enabled by the web.enabled field in the settings file.".to_string(),
            ),
            WebStartError::Bind { port, kind, .. } => Some(bind_advice(*port, *kind)),
            WebStartError::Random(_) => None,
        }
    }
}

fn bind_line(port: u16, kind: std::io::ErrorKind, reason: &str) -> String {
    let tail = format!("{NET_NAME} is running without the page");
    if kind == std::io::ErrorKind::AddrInUse {
        return format!(
            "the page didn't start: {} — {tail}; another port: {}",
            port_phrase(port, kind),
            other_port_hint(port)
        );
    }
    format!(
        "the page didn't start: {} ({reason}) — {tail}",
        port_phrase(port, kind)
    )
}

fn bind_advice(port: u16, kind: std::io::ErrorKind) -> String {
    if kind == std::io::ErrorKind::AddrInUse {
        return format!("Another port: {}.", other_port_hint(port));
    }
    "The --web-port N flag changes the port.".to_string()
}

impl std::error::Error for WebStartError {}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
