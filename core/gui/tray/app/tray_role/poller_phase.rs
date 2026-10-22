//! The poller phase and transitions on a node reply: the decision from `api::start_policy`,
//! starting our own node, a tick per phase. The "Stop" and "Quit" procedures are in `poller_stop.rs`.

use std::process::Child;

use tokio::time::Instant;

use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitMenu};
use crate::gui::tray::app::local_api::{ApiError, Reply};
use crate::gui::tray::app::start_note::StartNote;
use crate::gui::tray::app::view::{self, NodeState, PageState, TrayColor, View};

use super::api::{
    child_status_text, fetch_reply, reply_summary, start_policy, NodeRequest, StartDecision,
    KILLED_BY_SIGNAL_TEXT,
};
use super::poller::{Poller, QuitCause, QuitTriggers};
use super::views::{grey_view, views_from_state_json, ABSENT_VIEW, RUNNING_UNAVAILABLE_VIEW};

/// The poller phase.
pub(super) enum Phase {
    /// Before the first reply at startup.
    Deciding,
    /// Our own node: started by this icon.
    OwnNode {
        child: Child,
        started_at: Instant,
        seen_200: bool,
    },
    /// A foreign node: not started by the icon (or a new icon next to an old node of ours).
    ForeignNode {
        failed_poll_count: u32,
    },
    Absent,
    PortBusy,
    Disabled,
}

impl Poller {
    pub(super) fn say(&self, line: String) {
        (self.log)(line);
    }

    /// A view change: if the new one differs from the last, it goes to the tray and into the log. A grey view means the node is not
    /// running: the exits submenu is turned off in the same step, so there is no need to call `show_exits` separately on
    /// every grey view — all `View`s assembled by hand in the module are always grey, and
    /// green/red come only from `views_from_state_json`.
    pub(super) fn show(&mut self, view: View) {
        self.forget_start_note(view.node);
        if self.last_view != Some(view) {
            let previous = self.last_view.replace(view);
            (self.send_view)(view);
            self.say_view(view, previous);
        }
        if view.color == TrayColor::Grey {
            self.show_exits(disabled_exit_menu(self.proxy.clone()));
        }
    }

    /// A log line about a view change; a change of only the speed disc produces no line (the icon
    /// is redrawn, the log is not cluttered).
    fn say_view(&self, view: View, previous: Option<View>) {
        if previous.map(|p| View { disk: None, ..p }) == Some(View { disk: None, ..view }) {
            return;
        }
        self.say(format!(
            "{:?}; {}; {}",
            view.color,
            view::page_text(view),
            view::node_state_phrase(view.node)
        ));
    }

    /// The exits submenu: if the new one differs from the last, it goes to the tray.
    pub(super) fn show_exits(&mut self, menu: ExitMenu) {
        if self.last_exit_menu == Some(menu.clone()) {
            return;
        }
        self.last_exit_menu = Some(menu.clone());
        (self.send_exit_menu)(menu);
    }

    /// A transition on the outcome of `start_policy`. `allow_spawn` is the decision at startup and
    /// "Start Mesh Town"; when polling without a node, `Spawn` means "no node".
    pub(super) fn enter(
        &mut self,
        reply: Result<Reply, ApiError>,
        allow_spawn: bool,
        was_busy: bool,
    ) -> Phase {
        match start_policy(&reply) {
            StartDecision::Spawn if allow_spawn => self.start_own(),
            StartDecision::Spawn => {
                self.show(ABSENT_VIEW);
                Phase::Absent
            }
            StartDecision::Attach => {
                self.say(format!("node is already running on the port {}", self.port));
                if let Ok(Reply::Ok(state)) = &reply {
                    let (menu, view) = views_from_state_json(state, &self.proxy);
                    self.show_exits(menu);
                    self.show(view);
                }
                Phase::ForeignNode {
                    failed_poll_count: 0,
                }
            }
            StartDecision::PortBusy => {
                if !was_busy {
                    self.say(format!(
                        "port {} is held by something other than a node: {}",
                        self.port,
                        reply_summary(&reply)
                    ));
                }
                self.show(grey_view(
                    PageState::PortBusy(self.port),
                    NodeState::Unknown,
                ));
                Phase::PortBusy
            }
        }
    }

    pub(super) fn start_own(&mut self) -> Phase {
        if let Some(port) = (self.start_fail.busy_proxy)() {
            return self.refuse_start(port);
        }
        match (self.spawn)() {
            Ok(child) => {
                self.say(format!("node started, pid {}", child.id()));
                self.show(grey_view(PageState::Starting, NodeState::Starting));
                Phase::OwnNode {
                    child,
                    started_at: Instant::now(),
                    seen_200: false,
                }
            }
            Err(e) => {
                self.say(format!("node failed to start: {e}"));
                self.note_start(StartNote::Idle);
                self.show(ABSENT_VIEW);
                Phase::Absent
            }
        }
    }

    /// A poll tick; the request races against quit. Dispatch by phase is in `tick_own`/`tick_foreign`.
    pub(super) async fn tick(
        &mut self,
        phase: &mut Phase,
        quit_triggers: &mut QuitTriggers,
    ) -> Result<(), QuitCause> {
        match phase {
            Phase::OwnNode { .. } => self.tick_own(phase, quit_triggers).await,
            Phase::ForeignNode { .. } => self.tick_foreign(phase, quit_triggers).await,
            Phase::Absent | Phase::PortBusy => {
                let was_busy = matches!(phase, Phase::PortBusy);
                let reply = quit_triggers
                    .race_with_quit(fetch_reply(
                        self.port,
                        &self.token_path,
                        NodeRequest::State,
                        None,
                    ))
                    .await?;
                *phase = self.enter(reply, false, was_busy);
                Ok(())
            }
            Phase::Deciding | Phase::Disabled => Ok(()),
        }
    }

    /// `Phase::OwnNode` on a tick: the child exited — "Absent"; otherwise a request and a view by patience/200.
    async fn tick_own(
        &mut self,
        phase: &mut Phase,
        quit_triggers: &mut QuitTriggers,
    ) -> Result<(), QuitCause> {
        let Phase::OwnNode {
            child,
            started_at,
            seen_200,
        } = phase
        else {
            unreachable!("tick_own is only for Phase::Own")
        };
        let exited = match child.try_wait() {
            Ok(Some(status)) => Some((child_status_text(&status), Some(status))),
            Err(_) => Some((KILLED_BY_SIGNAL_TEXT.to_string(), None)),
            Ok(None) => None,
        };
        if let Some((code, status)) = exited {
            self.say(format!("node stopped, code {code}"));
            self.note_child_exit(status);
            *phase = Phase::Absent;
            self.show(ABSENT_VIEW);
            return Ok(());
        }
        let reply = quit_triggers
            .race_with_quit(fetch_reply(
                self.port,
                &self.token_path,
                NodeRequest::State,
                None,
            ))
            .await?;
        let v = match &reply {
            Ok(Reply::Ok(state)) => {
                *seen_200 = true;
                let (menu, view) = views_from_state_json(state, &self.proxy);
                self.show_exits(menu);
                view
            }
            _ if *seen_200 => RUNNING_UNAVAILABLE_VIEW,
            _ if started_at.elapsed() > self.timing.own_unavailable_after => {
                grey_view(PageState::Unavailable, NodeState::Starting)
            }
            _ => grey_view(PageState::Starting, NodeState::Starting),
        };
        self.show(v);
        Ok(())
    }

    /// `Phase::ForeignNode` on a tick: a request and a counter of consecutive failures.
    async fn tick_foreign(
        &mut self,
        phase: &mut Phase,
        quit_triggers: &mut QuitTriggers,
    ) -> Result<(), QuitCause> {
        let Phase::ForeignNode { failed_poll_count } = phase else {
            unreachable!("tick_foreign is only for Phase::Foreign")
        };
        let reply = quit_triggers
            .race_with_quit(fetch_reply(
                self.port,
                &self.token_path,
                NodeRequest::State,
                None,
            ))
            .await?;
        match &reply {
            Ok(Reply::Ok(state)) => {
                *failed_poll_count = 0;
                let (menu, view) = views_from_state_json(state, &self.proxy);
                self.show_exits(menu);
                self.show(view);
            }
            Err(_) => {
                *failed_poll_count += 1;
                if *failed_poll_count >= self.timing.foreign_gone_after_failures {
                    self.say("node stopped".to_string());
                    *phase = Phase::Absent;
                    self.show(ABSENT_VIEW);
                } else {
                    self.show(RUNNING_UNAVAILABLE_VIEW);
                }
            }
            // 401 and other replies mean the node is in place, the counter does not grow.
            Ok(_) => self.show(RUNNING_UNAVAILABLE_VIEW),
        }
        Ok(())
    }
}
