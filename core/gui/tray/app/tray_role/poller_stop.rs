//! The "Stop" procedure and shutdown on "Quit"/signal: for our own node — `POST` until
//! it is accepted and `try_wait` until the deadline, otherwise `kill`; for a foreign one — `POST` once and wait for a rejected
//! connection. Phases and the tick are in `poller_phase.rs`.

use std::process::Child;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

use crate::gui::tray::app::local_api::{ApiError, Reply};
use crate::gui::tray::app::view::View;
use crate::gui::tray::NodeCmd;

use super::api::{
    child_status_text, fetch_reply, reply_summary, NodeRequest, KILLED_BY_SIGNAL_TEXT,
};
use super::poller::{Poller, QuitCause};
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

/// Whether the node should get SIGTERM before the HTTP stop: only a node that has not served a page yet (it may
/// ignore the request for a while), and only where the signal exists.
fn signals_first(seen_200: bool, unix: bool) -> bool {
    unix && !seen_200
}

/// SIGTERM to the child; the node shuts down by it on its own. Nothing happens off Unix.
#[cfg(unix)]
fn send_term(child: &Child) -> Result<(), std::io::Error> {
    // SAFETY: plain `kill(2)` with the pid of a child that `try_wait` has just reported as not yet reaped.
    match unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

#[cfg(not(unix))]
fn send_term(_child: &Child) -> Result<(), std::io::Error> {
    Ok(())
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
                // The button keeps the HTTP-only path in every phase.
                let end = self.stop_own(&mut child, true).await;
                let code = match end {
                    OwnStopOutcome::Gone(code) => code,
                    OwnStopOutcome::Killed => KILLED_BY_SIGNAL_TEXT.to_string(),
                };
                let line = format!("node stopped via \"Stop Mesh Town\", code {code}");
                self.finish_stop(cmds, Some(line), ABSENT_VIEW);
                Phase::Absent
            }
            Phase::ForeignNode { .. } => {
                self.show(STOPPING_VIEW);
                let gone = self.stop_foreign().await;
                if gone {
                    let line = "node stopped via \"Stop Mesh Town\"".to_string();
                    self.finish_stop(cmds, Some(line), ABSENT_VIEW);
                    Phase::Absent
                } else {
                    self.finish_stop(cmds, None, RUNNING_UNAVAILABLE_VIEW);
                    Phase::ForeignNode {
                        failed_poll_count: 0,
                    }
                }
            }
            other => other,
        }
    }

    /// The end of the "Stop" procedure: pending commands are discarded, then the outcome line (if any) and view.
    fn finish_stop(
        &mut self,
        cmds: &mut UnboundedReceiver<NodeCmd>,
        line: Option<String>,
        view: View,
    ) {
        discard_pending_cmds(cmds);
        if let Some(line) = line {
            self.say(line);
        }
        self.show(view);
    }

    /// The "Stop" procedure for our own node: `POST` until accepted, `try_wait` until the deadline, otherwise `kill`.
    /// A node that has not served a page yet (`seen_200` is false) first gets SIGTERM, once.
    async fn stop_own(&mut self, child: &mut Child, seen_200: bool) -> OwnStopOutcome {
        let deadline = Instant::now() + self.timing.stop_wait;
        let mut term_pending = signals_first(seen_200, cfg!(unix));
        let mut accepted = false;
        let mut last_post: Option<Instant> = None;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return OwnStopOutcome::Gone(child_status_text(&status)),
                Err(_) => return OwnStopOutcome::Gone(KILLED_BY_SIGNAL_TEXT.to_string()),
                Ok(None) => {}
            }
            if std::mem::take(&mut term_pending) {
                if let Err(e) = send_term(child) {
                    self.say(format!("could not send SIGTERM to the node: {e}"));
                }
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            if !accepted && last_post.is_none_or(|at| now.duration_since(at) >= STOP_POST_INTERVAL)
            {
                last_post = Some(now);
                let limit = (deadline - now).min(STOP_REQUEST_TIMEOUT_MAX);
                let reply =
                    fetch_reply(self.port, &self.token_path, NodeRequest::Stop, Some(limit)).await;
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
                self.say(format!(
                    "node did not accept the stop: {}",
                    reply_summary(&other)
                ));
                return false;
            }
        }
        self.wait_until_stopped(deadline).await
    }

    /// Waits until the foreign node refuses connections or the deadline passes. `true` means it went away.
    async fn wait_until_stopped(&mut self, deadline: Instant) -> bool {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            tokio::time::sleep(FOREIGN_STOP_POLL_INTERVAL.min(left)).await;
            // The last check, right at the deadline: a rejected connection arrives within the request limit.
            let left = deadline.saturating_duration_since(Instant::now());
            let limit = left
                .max(OWN_STOP_POLL_INTERVAL)
                .min(STOP_REQUEST_TIMEOUT_MAX);
            let reply =
                fetch_reply(self.port, &self.token_path, NodeRequest::State, Some(limit)).await;
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
            Phase::OwnNode {
                mut child,
                seen_200,
                ..
            } => {
                self.show(STOPPING_VIEW);
                match self.stop_own(&mut child, seen_200).await {
                    OwnStopOutcome::Gone(code) => {
                        self.say(format!("node stopped via \"Quit\", code {code}"));
                        0
                    }
                    OwnStopOutcome::Killed => {
                        self.say(format!(
                            "node stopped via \"Quit\", code {KILLED_BY_SIGNAL_TEXT}"
                        ));
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

#[cfg(test)]
#[path = "poller_stop_tests.rs"]
mod tests;
