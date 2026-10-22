//! Command channel and dispatcher: `TrayCmd`/`CmdTx`/`Core`, `channel()`/`run()`, `dispatch_tray_cmd()` — the only
//! place of logic on top of a command. It knows nothing about the node, the page or `meshtown`.

use std::sync::{Arc, Mutex};

use super::actions_ui::ActionsUi;
use super::frame_cache::FrameCache;
use super::menu_build::{build_icons, build_menu, install_menu_handler, TrayUi};
use super::open_url;
use super::spin::Spin;
use crate::app::proxy_state::{ProxyOff, ProxyState};
use crate::app::start_note::StartNote;
use crate::app::tray_actions::{InviteNote, ServeNote};
use crate::app::view::{start_enabled, stop_enabled};

/// Identifier of the "Open" menu item.
pub(crate) const MENU_OPEN: &str = "open";
/// Identifier of the "Quit" menu item.
pub(crate) const MENU_QUIT: &str = "quit";
/// Identifier of the inactive status line item.
pub(crate) const MENU_STATUS: &str = "status";
/// Identifier of the single node item (replaces the former pair "Start
/// node"/"Stop node").
pub(crate) const MENU_NODE: &str = "node";
/// Identifier of the inactive item with the browser proxy address.
pub(crate) const MENU_PROXY: &str = "proxy";
/// Identifier of the "Invite a member" item.
pub(crate) const MENU_INVITE: &str = "invite";
/// Identifier of the "Allow sites to open via this computer" checkbox.
pub(crate) const MENU_SERVE: &str = "serve";
/// Common id prefix of the leaves of the "Exit via…" submenu: the bare prefix without
/// a tail is the "As usual" item (clear the choice), the prefix + the node's `id_full` toggles that node.
pub(crate) const MENU_EXIT_PREFIX: &str = "exit:";

/// A command to the node poller from the menu items. Sent only by `Core::dispatch_tray_cmd`, read only by
/// the `tray_role` poller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCmd {
    Start,
    Stop,
    /// Choose/clear an exit by the node's `id_full` — a click on a submenu leaf with that id.
    ToggleExit(String),
    /// Clear the chosen exit — a click on "As usual".
    ClearExits,
    /// Copy the invite to the clipboard — a click on "Invite a member".
    CopyInvite,
    /// Turn `exit.enabled` on or off — a click on the checkbox "Allow sites to open via this computer".
    SetExitServer(bool),
}

/// Sender of commands to the node poller.
pub type NodeCmdTx = tokio::sync::mpsc::UnboundedSender<NodeCmd>;

/// A command to the tray: update the icon/menu from the node snapshot, update the "Exit via…" submenu, a click on a
/// menu item, a spinner tick, end the process with a code.
pub enum TrayCmd {
    Update(crate::app::view::View),
    ExitMenu(crate::app::exit_menu::ExitMenu),
    Menu(String),
    /// What the invite item shows (the "copied" label, a refusal reason, or idle).
    Invite(InviteNote),
    /// Note on the "Allow sites to open via this computer" checkbox (edit failure or idle).
    Serve(ServeNote),
    /// Why the node did not start (or `Idle`).
    StartFail(crate::app::start_note::StartNote),
    /// A spinner tick of the ticker of the given generation; stale ones are ignored.
    SpinTick(u64),
    QuitWithCode(i32),
}

/// A cloneable command sender. The `Sender` is under a `Mutex`, not bare: the closure of
/// `MenuEvent::set_event_handler` requires `Send + Sync`, while `mpsc::Sender` is only `Send`
/// (`muda-0.20.0/src/menu_event.rs:46`); the `Mutex` gives `Sync`, the `Arc` gives cheap shared ownership of
/// one channel when cloning.
#[derive(Clone)]
pub struct CmdTx {
    tx: Arc<Mutex<std::sync::mpsc::Sender<TrayCmd>>>,
    wake: fn(),
}

impl CmdTx {
    /// Puts a command into the channel and wakes the platform loop: lock, send, release the lock,
    /// wake. A broken channel (the loop has already exited) is not the caller's error: the command is lost.
    pub fn send(&self, cmd: TrayCmd) {
        {
            let sender = self.tx.lock().expect("CmdTx: sender mutex poisoned");
            let _ = sender.send(cmd);
        }
        (self.wake)();
    }
}

/// Gets the node page link at the moment of the "Open" click (`None` — there is no page right now).
pub type LinkFn = fn() -> Option<String>;

/// State of the platform loop: the command channel, its own sender (for `attach_ui`), the quit
/// signal, the page link, the sender of commands to the node poller, the raised icon (if any),
/// the last view/chosen exit/proxy — kept even without a raised icon, `apply_*`
/// remembers them in any case.
pub struct Core {
    rx: std::sync::mpsc::Receiver<TrayCmd>,
    tx: CmdTx,
    quit: crate::app::quit::QuitSignal,
    page_url_fn: LinkFn,
    node_cmd_tx: NodeCmdTx,
    ui: Option<TrayUi>,
    view: crate::app::view::View,
    route: crate::app::view::RouteLine,
    proxy: ProxyState,
    invite_note: InviteNote,
    serve_note: ServeNote,
    start_note: StartNote,
    serves_exit: bool,
    members: crate::app::snapshot::TrayMembers,
    spin: Spin,
}

impl Core {
    /// Raises the icon and the menu once: an icon cache by color, the menu, `MenuEvent::set_event_handler`.
    /// A repeated call is not expected — callers (02/05/06) call it exactly once.
    pub(crate) fn attach_ui(&mut self) -> Result<(), String> {
        let icons = build_icons()?;
        let exit = super::exit_menu::ExitUi::build()?;
        let actions = ActionsUi::build(self.invite_note);
        let (menu, status_item, open_item, node_item, proxy_item) =
            build_menu(self.view, &self.route, &self.proxy, exit.submenu(), &actions)?;

        let vote = super::vote_item::VoteUi::new(menu.clone());
        let icon = super::menu_build::view_icon(&icons, self.icon_key())?;
        let tray = tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_tooltip(self.menu_first_line())
            .build()
            .map_err(|e| e.to_string())?;

        install_menu_handler(self.tx.clone());

        self.ui = Some(TrayUi {
            tray_icon_handle: tray,
            status_item,
            open_item,
            node_item,
            proxy_item,
            icons,
            shown_icon_key: self.icon_key(),
            frames: FrameCache::new(),
            exit_submenu: exit,
            actions,
            vote,
        });
        self.refresh_ui();
        Ok(())
    }

    /// Handles all commands accumulated in the channel until it is empty. `QuitWithCode` stops handling and
    /// returns its code.
    #[cfg(any(test, windows, target_os = "macos"))]
    pub(crate) fn drain_commands(&mut self) -> Option<i32> {
        while let Ok(cmd) = self.rx.try_recv() {
            if let Some(code) = self.dispatch_tray_cmd(cmd) {
                return Some(code);
            }
        }
        None
    }

    /// Handles commands in a blocking way until `QuitWithCode`, returns its code. The channel does not break while
    /// `self` is alive — `Core` holds its own sender (the `tx` field).
    pub(crate) fn recv_blocking(&mut self) -> i32 {
        loop {
            let cmd = self
                .rx
                .recv()
                .expect("the channel is alive: Core holds its own CmdTx");
            if let Some(code) = self.dispatch_tray_cmd(cmd) {
                return code;
            }
        }
    }

    /// Removes the icon (Windows calls it before `exit_now` — otherwise a ghost stays in the tray).
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))] // only the Windows loop calls it (06)
    pub(crate) fn drop_ui(&mut self) {
        self.ui = None;
    }

    /// The only place of logic on top of a command.
    fn dispatch_tray_cmd(&mut self, cmd: TrayCmd) -> Option<i32> {
        match cmd {
            TrayCmd::Update(view) => {
                self.apply_update(view);
                None
            }
            TrayCmd::ExitMenu(menu) => {
                self.apply_exits(menu);
                None
            }
            TrayCmd::Menu(id) => {
                self.handle_menu(&id);
                None
            }
            TrayCmd::Invite(note) => {
                self.apply_invite(note);
                None
            }
            TrayCmd::Serve(note) => {
                self.apply_serve(note);
                None
            }
            TrayCmd::StartFail(note) => {
                self.apply_start_note(note);
                None
            }
            TrayCmd::SpinTick(generation) => {
                self.apply_spin_tick(generation);
                None
            }
            TrayCmd::QuitWithCode(code) => Some(code),
        }
    }

    /// `TrayCmd::Update`: remembers the view and updates the icon/menu from it.
    fn apply_update(&mut self, view: crate::app::view::View) {
        self.view = view;
        if let Some(ticker) = self.spin.follow(view) {
            ticker.spawn(self.tx.clone());
        }
        self.refresh_ui();
    }

    /// `TrayCmd::ExitMenu`: remembers the chosen exit and proxy, rebuilds the
    /// "Exit via…" submenu and updates the status line/proxy item.
    fn apply_exits(&mut self, menu: crate::app::exit_menu::ExitMenu) {
        self.route = menu.route.clone();
        self.proxy = menu.proxy.clone();
        self.serves_exit = menu.serves_exit;
        self.members = menu.members;
        if let Some(ui) = &mut self.ui {
            ui.actions.apply_exit_menu(&menu);
            ui.exit_submenu.apply_exit_menu(menu);
        }
        self.refresh_ui();
    }

    /// The `NSStatusItem` of the icon for the "hidden" check (`macos_hidden`); `None` until `attach_ui`.
    #[cfg(target_os = "macos")]
    pub(super) fn status_item(&self) -> Option<objc2::rc::Retained<objc2_app_kit::NSStatusItem>> {
        self.ui.as_ref()?.tray_icon_handle.ns_status_item()
    }
}

/// Builds the command channel and the platform loop state. `wake` is chosen by OS — the only
/// `cfg(target_os)` branch in this file.
pub fn channel(quit: crate::app::quit::QuitSignal, page_url_fn: LinkFn, node_cmd_tx: NodeCmdTx) -> (CmdTx, Core) {
    let (sender, rx) = std::sync::mpsc::channel();

    #[cfg(target_os = "macos")]
    let wake: fn() = super::macos::wake;
    #[cfg(target_os = "windows")]
    let wake: fn() = super::windows::wake;
    #[cfg(target_os = "linux")]
    let wake: fn() = super::linux::wake;

    let tx = CmdTx {
        tx: Arc::new(Mutex::new(sender)),
        wake,
    };
    let core = Core {
        rx,
        tx: tx.clone(),
        quit,
        page_url_fn,
        node_cmd_tx,
        ui: None,
        view: crate::app::view::View::initial(),
        route: crate::app::view::RouteLine::Direct,
        proxy: ProxyState::Off(ProxyOff::DisabledInSettings),
        invite_note: InviteNote::Idle,
        serve_note: ServeNote::Idle,
        start_note: StartNote::Idle,
        serves_exit: false,
        members: Default::default(),
        spin: Spin::new(),
    };
    (tx, core)
}

/// The platform tray loop: handles commands and never returns (exits through
/// `app::quit::exit_now`). The module for its OS is chosen by `use … as platform`.
pub fn run(core: Core) -> ! {
    #[cfg(target_os = "linux")]
    use super::linux as platform;
    #[cfg(target_os = "macos")]
    use super::macos as platform;
    #[cfg(target_os = "windows")]
    use super::windows as platform;
    platform::run(core)
}

/// The only output of the `tray` module: `[tray] …`, without the `say()` timestamp — `tray/*` does not
/// see the `meshtown` crate; the platform `run` prints it.
pub(super) fn log_tray(text: &str) {
    println!("[tray] {text}");
}

#[path = "core_menu.rs"]
mod menu;

#[path = "core_line.rs"]
mod line;

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
