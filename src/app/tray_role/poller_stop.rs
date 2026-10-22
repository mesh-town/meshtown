//! The "Stop" procedure and shutdown on "Quit"/signal: for our own node — `POST` until
//! it is accepted and `try_wait` until the deadline, otherwise `kill`; for a foreign one — `POST` once and wait for a rejected
//! connection. Phases and the tick are in `poller_phase.rs`.

use std::process::Child;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

use crate::app::local_api::{ApiError, Reply};
use crate::tray::NodeCmd;

use super::api::{child_status_text, reply_summary, fetch_reply, NodeRequest, KILLED_BY_SIGNAL_TEXT};
use super::poller::{QuitCause, Poller};
use super::poller_phase::Phase;
use super::views::{ABSENT_VIEW, RUNNING_UNAVAILABLE_VIEW, STOPPING_VIEW};

/// Ceiling for one request inside the "Stop" procedure (after that — the rest of the deadline). Above the time a
/// refused loopback connect takes on Windows (2–2.5 s: the refusal comes after SYN retransmits, not at once).
const STOP_REQUEST_TIMEOUT_MAX: Duration = Duration::from_secs(4);
const OWN_STOP_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// `POST /api/actions/stop` to our own node — no more often than once per this interval.
const STOP_POST_INTERVAL: Duration = Duration::from_secs(1);
/// Reaping after `kill` — no longer than this, in steps of `REAP_POLL_INTERVAL`.
const REAP_AFTER_KILL_TIMEOUT: Duration = Duration::from_secs(1);
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Step for checking whether a foreign node has gone after an accepted stop.
const FOREIGN_STOP_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How the "Stop" procedure for our own node ended.
enum OwnStopOutcome {
    /// Went away by itself; a code or `signal`.
    Gone(String),
    /// Did not go within the deadline — killed with `kill`.
    Killed,
}

/// Discards commands that arrived during the "Stop" procedure. Called right at the end of the procedure, before the
/// outcome line and view: a command sent after the outcome view is not discarded.
fn discard_pending_cmds(cmds: &mut UnboundedReceiver<NodeCmd>) {
    while cmds.try_recv().is_ok() {}
}

impl Poller {
    /// "Stop Mesh Town": the procedure with the "button" cause and a transition on its outcome. Commands that arrive
    /// during the procedure are discarded before the outcome line and view (`discard_pending_cmds`).
    pub(super) async fn stop_button(
        &mut self,
        phase: Phase,
        cmds: &mut UnboundedReceiver<NodeCmd>,
    ) -> Phase {
        match phase {
            Phase::OwnNode { mut child, .. } => {
                self.show(STOPPING_VIEW);
                let end = self.stop_own(&mut child).await;
                discard_pending_cmds(cmds);
                let code = match end {
                    OwnStopOutcome::Gone(code) => code,
                    OwnStopOutcome::Killed => KILLED_BY_SIGNAL_TEXT.to_string(),
                };
                self.say(format!("node stopped via \"Stop Mesh Town\", code {code}"));
                self.show(ABSENT_VIEW);
                Phase::Absent
            }
            Phase::ForeignNode { .. } => {
                self.show(STOPPING_VIEW);
                let gone = self.stop_foreign().await;
                discard_pending_cmds(cmds);
                if gone {
                    self.say("node stopped via \"Stop Mesh Town\"".to_string());
                    self.show(ABSENT_VIEW);
                    Phase::Absent
                } else {
                    self.show(RUNNING_UNAVAILABLE_VIEW);
                    Phase::ForeignNode { failed_poll_count: 0 }
                }
            }
            other => other,
        }
    }

    /// The "Stop" procedure for our own node: `POST` until accepted, `try_wait` until the deadline, otherwise `kill`.
    async fn stop_own(&mut self, child: &mut Child) -> OwnStopOutcome {
        let deadline = Instant::now() + self.timing.stop_wait;
        let mut accepted = false;
        let mut last_post: Option<Instant> = None;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return OwnStopOutcome::Gone(child_status_text(&status)),
                Err(_) => return OwnStopOutcome::Gone(KILLED_BY_SIGNAL_TEXT.to_string()),
                Ok(None) => {}
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            if !accepted && last_post.is_none_or(|at| now.duration_since(at) >= STOP_POST_INTERVAL) {
                last_post = Some(now);
                let limit = (deadline - now).min(STOP_REQUEST_TIMEOUT_MAX);
                let reply = fetch_reply(self.port, &self.token_path, NodeRequest::Stop, Some(limit)).await;
                // `Refused` before the first 200 — the page is not up yet: keep trying.
                accepted = matches!(reply, Ok(Reply::Accepted | Reply::Stopping));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            tokio::time::sleep(OWN_STOP_POLL_INTERVAL.min(left)).await;
        }
        self.say("node did not stop within 10 s, killing it".to_string());
        let _ = child.kill();
        let reap_until = Instant::now() + REAP_AFTER_KILL_TIMEOUT;
        loop {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            if Instant::now() >= reap_until {
                break;
            }
            tokio::time::sleep(REAP_POLL_INTERVAL).await;
        }
        OwnStopOutcome::Killed
    }

    /// The "Stop" procedure for a foreign node: `POST` once, wait for a rejected connection. `true` means
    /// it went away, `false` means it stayed.
    async fn stop_foreign(&mut self) -> bool {
        let deadline = Instant::now() + self.timing.stop_wait;
        let limit = deadline
            .saturating_duration_since(Instant::now())
            .min(STOP_REQUEST_TIMEOUT_MAX);
        let reply = fetch_reply(self.port, &self.token_path, NodeRequest::Stop, Some(limit)).await;
        match reply {
            Err(ApiError::Refused) => return true,
            Ok(Reply::Accepted | Reply::Stopping) => {}
            other => {
                self.say(format!("node did not accept the stop: {}", reply_summary(&other)));
                return false;
            }
        }
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            tokio::time::sleep(FOREIGN_STOP_POLL_INTERVAL.min(left)).await;
            // The last check, right at the deadline: a rejected connection arrives within the request limit.
            let left = deadline.saturating_duration_since(Instant::now());
            let limit = left.max(OWN_STOP_POLL_INTERVAL).min(STOP_REQUEST_TIMEOUT_MAX);
            let reply = fetch_reply(self.port, &self.token_path, NodeRequest::State, Some(limit)).await;
            if matches!(reply, Err(ApiError::Refused)) {
                return true;
            }
            if Instant::now() >= deadline {
                self.say("node did not stop within 10 s".to_string());
                return false;
            }
        }
    }

    /// "Quit" and the signal, by the phase they caught us in.
    pub(super) async fn quit_tray(&mut self, by: QuitCause, phase: Phase) -> i32 {
        if by == QuitCause::Signal {
            self.say("signal: quit".to_string());
        }
        match phase {
            Phase::OwnNode { mut child, .. } => {
                self.show(STOPPING_VIEW);
                match self.stop_own(&mut child).await {
                    OwnStopOutcome::Gone(code) => {
                        self.say(format!("node stopped via \"Quit\", code {code}"));
                        0
                    }
                    OwnStopOutcome::Killed => {
                        self.say(format!("node stopped via \"Quit\", code {KILLED_BY_SIGNAL_TEXT}"));
                        1
                    }
                }
            }
            Phase::ForeignNode { .. } => {
                self.say("quit: node was not started by the icon, keeps running".to_string());
                0
            }
            Phase::Deciding | Phase::Absent | Phase::PortBusy | Phase::Disabled => 0,
        }
    }
}
