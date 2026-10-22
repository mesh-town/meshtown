//! The reason a node refused to start, for the tray menu: a trial check of the proxy port before launch and
//! parsing the tail of `meshtown.log` when our own node exited by itself. The poller sends the reason to the tray as a separate
//! command and erases it on any change of the node view.

use std::net::TcpListener;
use std::process::ExitStatus;

use crate::config::config_file::ListenAccess;
use crate::proxy::listen_ip;

use crate::gui::tray::app::start_note::{start_note_from, start_note_line, StartNote};
use crate::gui::tray::app::view::NodeState;

use super::poller::Poller;
use super::poller_phase::Phase;
use super::views::ABSENT_VIEW;

/// The environment for the checks and the last reason sent.
pub(super) struct StartFail {
    /// Production: `busy_proxy_port` for the proxy port the node will start with. The port and on/off state come
    /// from the settings, but `--socks-port`, `--socks-access`, `--socks` and `--no-socks` after `--`
    /// beat them (`run.rs::proxy_plan_for`, `run.rs::proxy_access_for`). `Some(port)` means the port is busy now; `None` means it is free or the proxy
    /// is off.
    pub(super) busy_proxy: Box<dyn Fn() -> Option<u16> + Send>,
    /// Production: `console::node_log_tail` — the tail of the node log, if there is one.
    pub(super) node_log: Box<dyn Fn() -> Option<String> + Send>,
    /// Production: `CmdTx::send(TrayCmd::StartFail)`.
    pub(super) send_start_note: Box<dyn Fn(StartNote) + Send>,
    pub(super) last_note: StartNote,
}

impl StartFail {
    pub(super) fn new(
        busy_proxy: Box<dyn Fn() -> Option<u16> + Send>,
        node_log: Box<dyn Fn() -> Option<String> + Send>,
        send_start_note: Box<dyn Fn(StartNote) + Send>,
    ) -> StartFail {
        StartFail {
            busy_proxy,
            node_log,
            send_start_note,
            last_note: StartNote::Idle,
        }
    }
}

/// The proxy port if it is busy now (`None` means the proxy is off or the port is free): a trial `bind`
/// on the address where the node would sit (`localhost` is the loopback, `localnet` is `0.0.0.0`) is closed immediately.
pub(super) fn busy_proxy_port(port: Option<u16>, access: ListenAccess) -> Option<u16> {
    let port = port?;
    TcpListener::bind((listen_ip(access), port))
        .is_err()
        .then_some(port)
}

impl Poller {
    /// A new reason goes to the tray and a line to the log; the same one does nothing.
    pub(super) fn note_start(&mut self, note: StartNote) {
        if self.start_fail.last_note == note {
            return;
        }
        self.start_fail.last_note = note.clone();
        let line = start_note_line(&note);
        (self.start_fail.send_start_note)(note);
        if let Some(line) = line {
            self.say(format!("reason in the menu: {line}"));
        }
    }

    /// Any view with a live node erases the reason; the grey "Absent" does not touch it.
    pub(super) fn forget_start_note(&mut self, node: NodeState) {
        if node != NodeState::Absent {
            self.note_start(StartNote::Idle);
        }
    }

    /// The proxy port is busy with another program: we do not start the node.
    pub(super) fn refuse_start(&mut self, port: u16) -> Phase {
        self.say(format!(
            "port {port} for the proxy is in use by another program — not starting the node"
        ));
        self.note_start(StartNote::ProxyBusy(port));
        self.show(ABSENT_VIEW);
        Phase::Absent
    }

    /// Our own node exited by itself: success is not a refusal; otherwise the reason comes from the code and the log tail.
    pub(super) fn note_child_exit(&mut self, status: Option<ExitStatus>) {
        if status.is_some_and(|s| s.success()) {
            return;
        }
        let log = (self.start_fail.node_log)();
        let code = status.and_then(|s| s.code());
        self.note_start(start_note_from(code, log.as_deref()));
    }
}

#[cfg(test)]
#[path = "start_fail_tests.rs"]
mod tests;
