//! The vocabulary shared by startup (`run.rs`) and the poller (`poller.rs`): timeouts and everything the poller
//! receives from outside. The `pub(super)` fields are what `run.rs` assembles into `PollEnv` and `poller.rs` unpacks.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Child;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::exit_menu::ExitMenu;
use crate::app::proxy_state::ProxyState;
use crate::app::tray_actions::{InviteNote, ServeNote};
use crate::app::view::View;
use crate::tray::NodeCmd;

use super::poller::{QuitTriggers, Poller};
use super::start_fail::StartFail;

/// The poller's timeouts; in production `PollTiming::PROD`, in tests shorter ones.
#[derive(Clone, Copy, Debug)]
pub(super) struct PollTiming {
    pub(super) poll_interval: Duration,
    /// Our own node with no 200 for longer than this means "The node page is unavailable".
    pub(super) own_unavailable_after: Duration,
    /// The timeout of the "Stop" procedure.
    pub(super) stop_wait: Duration,
    /// This many failures in a row mean the foreign node is gone.
    pub(super) foreign_gone_after_failures: u32,
    /// How long notes in the first menu line (the invite, a checkbox failure) stay visible.
    pub(super) note_visible_for: Duration,
}

impl PollTiming {
    pub(super) const PROD: PollTiming = PollTiming {
        poll_interval: Duration::from_secs(10),
        own_unavailable_after: Duration::from_secs(30),
        stop_wait: Duration::from_secs(10),
        foreign_gone_after_failures: 3,
        note_visible_for: Duration::from_secs(10),
    };
}

/// A one-shot future: "Quit" or a signal.
pub(super) type Once = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Writing a string to the clipboard; the failure reason is a ready constant, not text with a secret.
pub(super) type CopyFn = Arc<dyn Fn(&str) -> Result<(), &'static str> + Send + Sync>;

/// Everything the poller receives from outside; in production `run` assembles it, in tests the test does.
pub(super) struct PollEnv {
    pub(super) port: u16,
    pub(super) token_path: PathBuf,
    pub(super) web_enabled: bool,
    /// The proxy as the settings file and the tray flags name it: an address or the reason it is "off".
    pub(super) proxy: ProxyState,
    pub(super) spawn: Box<dyn FnMut() -> std::io::Result<Child> + Send>,
    /// Production: `CmdTx::send(TrayCmd::Update)`.
    pub(super) send_view: Box<dyn Fn(View) + Send>,
    /// Production: `CmdTx::send(TrayCmd::ExitMenu)`.
    pub(super) send_exit_menu: Box<dyn Fn(ExitMenu) + Send>,
    /// Production: `crate::tray::copy_to_clipboard` (supplied by `run.rs`).
    pub(super) copy_to_clipboard_fn: CopyFn,
    /// Production: `CmdTx::send(TrayCmd::Invite)`.
    pub(super) send_invite_note: Box<dyn Fn(InviteNote) + Send>,
    /// Production: `CmdTx::send(TrayCmd::Serve)`.
    pub(super) send_serve_note: Box<dyn Fn(ServeNote) + Send>,
    /// Production: the check of the settings' proxy port, the tail of `meshtown.log`, `CmdTx::send(TrayCmd::StartFail)`.
    pub(super) start_fail: StartFail,
    /// Production: `say("tray", …)`.
    pub(super) log: Box<dyn Fn(String) + Send>,
    pub(super) cmds: UnboundedReceiver<NodeCmd>,
    /// Production: `quit.wait()`.
    pub(super) quit: Once,
    /// Production: `wait_signal()`, then `quit.request()`.
    pub(super) signal: Once,
}

impl PollEnv {
    /// Environment → poller, the "web enabled" flag, the menu command queue and the exits ("Quit", the signal).
    pub(super) fn into_poller_parts(self, timing: PollTiming) -> (Poller, bool, UnboundedReceiver<NodeCmd>, QuitTriggers) {
        let PollEnv {
            port,
            token_path,
            web_enabled,
            proxy,
            spawn,
            send_view,
            send_exit_menu,
            copy_to_clipboard_fn,
            send_invite_note,
            send_serve_note,
            start_fail,
            log,
            cmds,
            quit,
            signal,
        } = self;
        let poller = Poller {
            port,
            token_path,
            proxy,
            spawn,
            send_view,
            send_exit_menu,
            log,
            last_view: None,
            last_exit_menu: None,
            copy_to_clipboard_fn,
            send_invite_note,
            send_serve_note,
            notes_visible_until: None,
            start_fail,
            timing,
        };
        (poller, web_enabled, cmds, QuitTriggers::new(signal, quit))
    }
}
