//! Main loop of the poller: the decision at startup, choosing the next event (signal/"Quit", menu command,
//! tick), and the exit code. Phases and transitions live in `poller_phase.rs`, the "Stop" and "Quit"
//! procedures in `poller_stop.rs`; this file only picks the event and dispatches it to them.

use std::future::Future;
use std::path::PathBuf;
use std::process::Child;
use std::task::Poll;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{sleep_until, Instant, MissedTickBehavior};

use crate::app::exit_menu::ExitMenu;
use crate::app::proxy_state::ProxyState;
use crate::app::tray_actions::{InviteNote, ServeNote};
use crate::app::view::{NodeState, PageState, View};
use crate::tray::NodeCmd;

use super::api::{fetch_reply, NodeRequest};
use super::poll_types::{CopyFn, Once, PollEnv, PollTiming};
use super::poller_exit::ExitEdit;
use super::poller_phase::Phase;
use super::start_fail::StartFail;
use super::views::grey_view;

/// "Quit" and the signal are one-shot futures; a branch is disabled once it fires.
pub(super) struct QuitTriggers {
    signal: Once,
    quit: Once,
    signal_seen: bool,
    quit_seen: bool,
    /// Fired during the first poll (`prime`) — handed out by the very next `next_quit_cause`.
    fired: Option<QuitCause>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QuitCause {
    Signal,
    Quit,
}

impl QuitTriggers {
    pub(super) fn new(signal: Once, quit: Once) -> QuitTriggers {
        QuitTriggers {
            signal,
            quit,
            signal_seen: false,
            quit_seen: false,
            fired: None,
        }
    }

    /// First poll of the signal future — installs the handlers right away rather than at the first `select!`.
    pub(super) async fn prime(&mut self) {
        let signal = &mut self.signal;
        let ready =
            std::future::poll_fn(|cx| Poll::Ready(signal.as_mut().poll(cx).is_ready())).await;
        if ready {
            self.signal_seen = true;
            self.fired = Some(QuitCause::Signal);
        }
    }

    pub(super) fn has_pending_trigger(&self) -> bool {
        self.fired.is_some() || !(self.signal_seen && self.quit_seen)
    }

    /// The signal first, then "Quit" (`biased`); returns nothing once both have fired.
    pub(super) async fn next_quit_cause(&mut self) -> QuitCause {
        if let Some(by) = self.fired.take() {
            return by;
        }
        tokio::select! {
            biased;
            _ = &mut self.signal, if !self.signal_seen => {
                self.signal_seen = true;
                QuitCause::Signal
            }
            _ = &mut self.quit, if !self.quit_seen => {
                self.quit_seen = true;
                QuitCause::Quit
            }
            else => std::future::pending::<QuitCause>().await,
        }
    }

    /// Races a request against quit: if quit fires, the request is dropped.
    pub(super) async fn race_with_quit<T>(&mut self, fut: impl Future<Output = T>) -> Result<T, QuitCause> {
        tokio::select! {
            biased;
            by = self.next_quit_cause() => Err(by),
            value = fut => Ok(value),
        }
    }
}

/// An event of the main loop.
enum PollEvent {
    Quit(QuitCause),
    Cmd(Option<NodeCmd>),
    NotesExpired,
    Tick,
}

pub(super) struct Poller {
    pub(super) port: u16,
    pub(super) token_path: PathBuf,
    /// Proxy from the settings and tray flags, for the grey views and for the "off" reason on a live node.
    pub(super) proxy: ProxyState,
    pub(super) spawn: Box<dyn FnMut() -> std::io::Result<Child> + Send>,
    pub(super) send_view: Box<dyn Fn(View) + Send>,
    pub(super) send_exit_menu: Box<dyn Fn(ExitMenu) + Send>,
    pub(super) log: Box<dyn Fn(String) + Send>,
    pub(super) last_view: Option<View>,
    pub(super) last_exit_menu: Option<ExitMenu>,
    pub(super) copy_to_clipboard_fn: CopyFn,
    pub(super) send_invite_note: Box<dyn Fn(InviteNote) + Send>,
    pub(super) send_serve_note: Box<dyn Fn(ServeNote) + Send>,
    /// When both notes (invite, checkbox failure) go away; `None` means no notes are on screen.
    pub(super) notes_visible_until: Option<Instant>,
    pub(super) start_fail: StartFail,
    pub(super) timing: PollTiming,
}

/// Poll timer: the first tick comes after `timing.poll_interval`, missed ticks are delayed.
fn poll_ticker(timing: PollTiming) -> tokio::time::Interval {
    let mut ticker = tokio::time::interval_at(Instant::now() + timing.poll_interval, timing.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ticker
}

/// The poller: phases, menu commands, "Quit" and the signal. Returns the exit code and does not end the process.
pub(super) async fn poll_node(env: PollEnv, timing: PollTiming) -> i32 {
    let (mut poller, web_enabled, mut cmds, mut quit_triggers) = env.into_poller_parts(timing);
    quit_triggers.prime().await;
    poller.show(View::initial());

    // The decision.
    let mut phase = match decide_initial_phase(&mut poller, &mut quit_triggers, web_enabled).await {
        Ok(phase) => phase,
        Err(code) => return code,
    };

    let mut ticker = poll_ticker(timing);
    let mut cmds_open = true;
    loop {
        phase = match step(
            &mut poller,
            phase,
            &mut quit_triggers,
            &mut cmds,
            &mut cmds_open,
            &mut ticker,
        )
        .await
        {
            Ok(phase) => phase,
            Err(code) => return code,
        };
    }
}

/// The first decision: turned off in the settings — a grey "Off" view, otherwise a state request.
async fn decide_initial_phase(
    poller: &mut Poller,
    quit_triggers: &mut QuitTriggers,
    web_enabled: bool,
) -> Result<Phase, i32> {
    if !web_enabled {
        poller.show(grey_view(PageState::Disabled, NodeState::Unknown));
        return Ok(Phase::Disabled);
    }
    match quit_triggers
        .race_with_quit(fetch_reply(poller.port, &poller.token_path, NodeRequest::State, None))
        .await
    {
        Ok(reply) => Ok(poller.enter(reply, true, false)),
        Err(by) => Err(poller.quit_tray(by, Phase::Deciding).await),
    }
}

/// One turn of the loop: pick an event and dispatch it to a phase transition or an exit code.
async fn step(
    poller: &mut Poller,
    mut phase: Phase,
    quit_triggers: &mut QuitTriggers,
    cmds: &mut UnboundedReceiver<NodeCmd>,
    cmds_open: &mut bool,
    ticker: &mut tokio::time::Interval,
) -> Result<Phase, i32> {
    let ticking = !matches!(phase, Phase::Disabled);
    let until = poller.notes_visible_until;
    let ev = tokio::select! {
        biased;
        by = quit_triggers.next_quit_cause(), if quit_triggers.has_pending_trigger() => PollEvent::Quit(by),
        cmd = cmds.recv(), if *cmds_open => PollEvent::Cmd(cmd),
        _ = sleep_until(until.unwrap_or_else(Instant::now)), if until.is_some() => PollEvent::NotesExpired,
        _ = ticker.tick(), if ticking => PollEvent::Tick,
        else => PollEvent::Quit(QuitCause::Quit),
    };
    match ev {
        PollEvent::Quit(by) => Err(poller.quit_tray(by, phase).await),
        // `Core` is destroyed — the branch is disabled, otherwise the loop would spin idle.
        PollEvent::Cmd(None) => {
            *cmds_open = false;
            Ok(phase)
        }
        PollEvent::Cmd(Some(cmd)) => {
            let edits = changes_node_state(&cmd);
            let next = on_cmd(poller, phase, quit_triggers, cmds, cmd).await;
            if edits {
                // Polling runs every 10 s: after a node edit the next tick comes right away, not after the period.
                ticker.reset_immediately();
            }
            next
        }
        PollEvent::NotesExpired => {
            poller.clear_expired_notes();
            Ok(phase)
        }
        PollEvent::Tick => match poller.tick(&mut phase, quit_triggers).await {
            Ok(()) => Ok(phase),
            Err(by) => Err(poller.quit_tray(by, phase).await),
        },
    }
}

/// Whether the command changes node settings — the menu view must converge to the fact without waiting for a tick.
fn changes_node_state(cmd: &NodeCmd) -> bool {
    matches!(
        cmd,
        NodeCmd::ToggleExit(_) | NodeCmd::ClearExits | NodeCmd::SetExitServer(_)
    )
}

/// A tray menu command: dispatched by phase; outside the phase it needs, the command is silently dropped.
async fn on_cmd(
    poller: &mut Poller,
    phase: Phase,
    quit_triggers: &mut QuitTriggers,
    cmds: &mut UnboundedReceiver<NodeCmd>,
    cmd: NodeCmd,
) -> Result<Phase, i32> {
    let online = matches!(phase, Phase::OwnNode { .. } | Phase::ForeignNode { .. });
    match cmd {
        NodeCmd::Start if matches!(phase, Phase::Absent) => {
            match quit_triggers
                .race_with_quit(fetch_reply(poller.port, &poller.token_path, NodeRequest::State, None))
                .await
            {
                Ok(reply) => Ok(poller.enter(reply, true, false)),
                Err(by) => Err(poller.quit_tray(by, phase).await),
            }
        }
        NodeCmd::Start => Ok(phase),
        // Commands that arrive during the procedure are dropped by `stop_button` — before the result view.
        NodeCmd::Stop if online => Ok(poller.stop_button(phase, cmds).await),
        NodeCmd::Stop => Ok(phase),
        NodeCmd::ToggleExit(id) if online => {
            poller.send_exit_edit(ExitEdit::Toggle(id)).await;
            Ok(phase)
        }
        NodeCmd::ToggleExit(_) => Ok(phase),
        NodeCmd::ClearExits if online => {
            poller.send_exit_edit(ExitEdit::Clear).await;
            Ok(phase)
        }
        NodeCmd::ClearExits => Ok(phase),
        NodeCmd::CopyInvite if online => {
            poller.copy_invite().await;
            Ok(phase)
        }
        NodeCmd::CopyInvite => Ok(phase),
        NodeCmd::SetExitServer(on) if online => {
            poller.send_exit_server(on).await;
            Ok(phase)
        }
        NodeCmd::SetExitServer(_) => Ok(phase),
    }
}

#[cfg(test)]
#[path = "poller_test_support.rs"]
pub(super) mod test_support;

#[cfg(all(test, unix))]
#[path = "poller_own_tests.rs"]
mod own_tests;

#[cfg(test)]
#[path = "poller_foreign_tests.rs"]
mod foreign_tests;

#[cfg(test)]
#[path = "poller_misc_tests.rs"]
mod misc_tests;
