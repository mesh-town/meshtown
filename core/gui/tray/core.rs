//! Command channel and dispatcher: `TrayCmd`/`CmdTx`/`Core`, `channel()`/`run()`, `dispatch_tray_cmd()` — the only
//! place of logic on top of a command. It knows nothing about the node, the page or `meshtown`.

use std::sync::{Arc, Mutex};

use super::actions_ui::ActionsUi;
use super::backend::TrayBackend;
use super::frame_cache::FrameCache;
use super::menu_build::{build_icons, build_menu, install_menu_handler, TrayUi};
use super::nets_click::{menu_facts, MenuFacts};
use super::spin::Spin;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::start_note::StartNote;
use crate::gui::tray::app::tray_actions::{InviteNote, ServeNote};
use crate::gui::tray::app::view::{start_enabled, stop_enabled};

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

pub(crate) use super::node_cmd::{NodeCmd, NodeCmdTx};

/// A command to the tray: update the icon/menu from the node snapshot, update the "Exit via…" submenu, a click on a
/// menu item, a spinner tick, end the process with a code.
pub(crate) enum TrayCmd {
    Update(crate::gui::tray::app::view::View),
    ExitMenu(crate::gui::tray::app::exit_menu::ExitMenu),
    Menu(String),
    /// What the invite item shows (the "copied" label, a refusal reason, or idle).
    Invite(InviteNote),
    /// Note on the "Allow sites to open via this computer" checkbox (edit failure or idle).
    Serve(ServeNote),
    /// Why the node did not start (or `Idle`).
    StartFail(crate::gui::tray::app::start_note::StartNote),
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
    tray: &'static dyn TrayBackend,
}

impl CmdTx {
    /// Puts a command into the channel and wakes the platform loop: lock, send, release the lock,
    /// wake. A broken channel (the loop has already exited) is not the caller's error: the command is lost.
    pub fn send(&self, cmd: TrayCmd) {
        {
            let sender = self.tx.lock().expect("CmdTx: sender mutex poisoned");
            let _ = sender.send(cmd);
        }
        self.tray.wake();
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
    quit: crate::gui::tray::app::quit::QuitSignal,
    page_url_fn: LinkFn,
    node_cmd_tx: NodeCmdTx,
    tray: &'static dyn TrayBackend,
    ui: Option<TrayUi>,
    view: crate::gui::tray::app::view::View,
    route: crate::gui::tray::app::view::RouteLine,
    proxy: ProxyState,
    invite_note: InviteNote,
    serve_note: ServeNote,
    start_note: StartNote,
    serves_exit: bool,
    members: crate::gui::tray::app::snapshot::TrayMembers,
    spin: Spin,
    /// The network the flat menu works with; `None` is the main one.
    flat_slot: Option<String>,
    facts: MenuFacts,
    /// The network the last "Invite a member" click was for.
    invite_slot: Option<String>,
    /// Names of the shown networks that have no connection.
    offline_nets: Vec<String>,
    /// Whether a ban vote (not only a join request) is pending in some shown network.
    removal_vote: bool,
    /// The last applied "Exit via…" model, for the optimistic re-application on a click.
    last_exit: Option<crate::gui::tray::app::exit_menu::ExitMenu>,
}

impl Core {
    /// Raises the icon and the menu once: an icon cache by color, the menu, `MenuEvent::set_event_handler`.
    /// A repeated call is not expected — callers (02/05/06) call it exactly once.
    pub fn attach_ui(&mut self) -> Result<(), String> {
        let icons = build_icons()?;
        let exit = super::exit_menu::ExitUi::build()?;
        let mut actions = ActionsUi::build(self.invite_note);
        let (menu, status_item, open_item, node_item, proxy_item) = build_menu(
            self.view,
            &self.route,
            &self.proxy,
            exit.submenu(),
            &mut actions,
        )?;

        let vote = super::vote_item::VoteUi::new(menu.clone());
        let nets = super::nets_ui::NetsUi::new(menu.clone());
        let extras = super::extras_ui::ExtrasUi::new(menu.clone());
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
            nets,
            extras,
        });
        self.refresh_ui();
        Ok(())
    }

    /// Handles all commands accumulated in the channel until it is empty. `QuitWithCode` stops handling and
    /// returns its code.
    pub fn drain_commands(&mut self) -> Option<i32> {
        while let Ok(cmd) = self.rx.try_recv() {
            if let Some(code) = self.dispatch_tray_cmd(cmd) {
                return Some(code);
            }
        }
        None
    }

    /// Handles commands in a blocking way until `QuitWithCode`, returns its code. The channel does not break while
    /// `self` is alive — `Core` holds its own sender (the `tx` field).
    pub fn recv_blocking(&mut self) -> i32 {
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
    pub fn drop_ui(&mut self) {
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
    fn apply_update(&mut self, view: crate::gui::tray::app::view::View) {
        self.view = view;
        if let Some(ticker) = self.spin.follow(view) {
            ticker.spawn(self.tx.clone());
        }
        self.refresh_ui();
    }

    /// `TrayCmd::ExitMenu`: remembers the chosen exit and proxy, rebuilds the
    /// "Exit via…" submenu and updates the status line/proxy item.
    fn apply_exits(&mut self, menu: crate::gui::tray::app::exit_menu::ExitMenu) {
        self.last_exit = Some(menu.clone());
        self.route = menu.route.clone();
        self.proxy = menu.proxy.clone();
        self.serves_exit = menu.serves_exit;
        self.members = menu.members;
        self.flat_slot = menu.slot.clone();
        self.facts = menu_facts(&menu);
        self.offline_nets = menu
            .nets
            .iter()
            .filter(|n| !n.online && !n.joining)
            .map(|n| n.label.clone())
            .collect();
        self.removal_vote = menu.nets.iter().any(|n| n.voting > 0);
        let note = self.net_invite_note();
        if let Some(ui) = &mut self.ui {
            ui.actions.apply_exit_menu(&menu);
            let shown = note
                .as_ref()
                .map(|(slot, text)| (slot.as_str(), text.as_str()));
            ui.nets.apply(&menu, shown, &mut ui.actions);
            ui.extras.apply_notices(&menu.notices);
            ui.extras.apply_add(&menu);
            ui.exit_submenu.apply_exit_menu(menu);
        }
        self.refresh_ui();
    }

    /// The raised tray icon, for platform code that needs the native handle; `None` until `attach_ui`.
    pub fn tray_icon(&self) -> Option<&tray_icon::TrayIcon> {
        self.ui.as_ref().map(|ui| &ui.tray_icon_handle)
    }
}

/// Builds the command channel and the platform loop state; the platform is reached only through `tray`.
pub(crate) fn channel(
    quit: crate::gui::tray::app::quit::QuitSignal,
    page_url_fn: LinkFn,
    node_cmd_tx: NodeCmdTx,
    tray: &'static dyn TrayBackend,
) -> (CmdTx, Core) {
    let (sender, rx) = std::sync::mpsc::channel();

    let tx = CmdTx {
        tx: Arc::new(Mutex::new(sender)),
        tray,
    };
    let core = Core {
        rx,
        tx: tx.clone(),
        quit,
        page_url_fn,
        node_cmd_tx,
        tray,
        ui: None,
        view: crate::gui::tray::app::view::View::initial(),
        route: crate::gui::tray::app::view::RouteLine::Direct,
        proxy: ProxyState::Off(ProxyOff::DisabledInSettings),
        invite_note: InviteNote::Idle,
        serve_note: ServeNote::Idle,
        start_note: StartNote::Idle,
        serves_exit: false,
        members: Default::default(),
        spin: Spin::new(),
        flat_slot: None,
        facts: MenuFacts::default(),
        invite_slot: None,
        offline_nets: Vec::new(),
        removal_vote: false,
        last_exit: None,
    };
    (tx, core)
}

/// The platform tray loop: handles commands and never returns (exits through
/// `app::quit::exit_now`). The loop belongs to the backend the channel was built with.
pub(crate) fn run(core: Core) -> ! {
    let tray = core.tray;
    tray.run(core)
}

/// The only output of the `tray` module: `[tray] …`, without the `say()` timestamp — `tray/*` does not
/// see the `meshtown` crate; the platform `run` prints it.
pub fn log_tray(text: &str) {
    println!("[tray] {text}");
}

#[path = "core_menu.rs"]
mod menu;

#[path = "core_line.rs"]
mod line;

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
