//! Pure construction of the tray UI: icons by color, the menu items, the menu click handler, the raised
//! icon and its parts (`TrayUi`). It holds no `Core` state — called from `Core::attach_ui` and
//! `Core::refresh_ui` (module `core`).

use super::actions_ui::ActionsUi;
use super::core::{CmdTx, TrayCmd, MENU_NODE, MENU_OPEN, MENU_PROXY, MENU_QUIT, MENU_STATUS};
use super::frame_cache::FrameCache;
use crate::gui::tray::app::members_note::alert_wanted;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::snapshot::TrayMembers;
use crate::gui::tray::app::tray_texts::{
    proxy_on_addr_text, proxy_on_host_port_text, quit_text, PROXY_OFF_FLAG_TEXT,
    PROXY_OFF_SETTINGS_TEXT,
};
use crate::gui::tray::app::view::{View, ICON_SIDE_PX};
use crate::gui::tray::app::view_picture::{icon_picture, picture_rgba, IconPicture, IdleRing};

/// The four tray icons by color (the index is `TrayColor as usize`, the order is `TRAY_COLORS`).
pub(super) fn build_icons() -> Result<[tray_icon::Icon; 4], String> {
    use crate::gui::tray::app::view::{icon_rgba, TRAY_COLORS};
    let icons = TRAY_COLORS
        .iter()
        .map(|&color| {
            tray_icon::Icon::from_rgba(icon_rgba(color), ICON_SIDE_PX, ICON_SIDE_PX)
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    icons
        .try_into()
        .map_err(|_| "the number of icons did not match TRAY_COLORS".to_string())
}

/// The icon for a picture: a plain one (no raster) comes from the `icons` cache by color, otherwise a new raster
/// (`picture_rgba`). A `from_rgba` error is returned as text; the caller keeps the previous icon.
pub(super) fn view_icon(
    icons: &[tray_icon::Icon; 4],
    picture: IconPicture,
) -> Result<tray_icon::Icon, String> {
    let Some(rgba) = picture_rgba(&picture, ICON_SIDE_PX) else {
        return Ok(icons[picture.color as usize].clone());
    };
    tray_icon::Icon::from_rgba(rgba, ICON_SIDE_PX, ICON_SIDE_PX).map_err(|e| e.to_string())
}

/// The picture for a view, the membership facts and the idle ring memory: the grey ring of a node that is not
/// running; the "vote waiting" mark replaces the disk.
pub(super) fn picture_for(view: View, members: TrayMembers, idle: IdleRing) -> IconPicture {
    icon_picture(view)
        .with_idle(view, idle)
        .with_alert(alert_wanted(members))
}

/// The text of the proxy menu item: address and port separately — in the browser
/// settings they are different fields. If the address cannot be parsed (no `:`) — the form "{addr} (SOCKS5 or HTTP)"; IPv6 in
/// brackets (`[::1]:1080`) — the address without brackets. The item is always inactive — a hint, not a switch.
pub(super) fn proxy_item_text(proxy: &ProxyState) -> String {
    match proxy {
        ProxyState::On(addr) => on_text(addr),
        ProxyState::Off(ProxyOff::DisabledInSettings) => PROXY_OFF_SETTINGS_TEXT.to_string(),
        ProxyState::Off(ProxyOff::NoSocksFlag) => PROXY_OFF_FLAG_TEXT.to_string(),
    }
}

/// The text of an enabled proxy by the `host:port` address.
fn on_text(addr: &str) -> String {
    let Some((host, port)) = addr.rsplit_once(':') else {
        return proxy_on_addr_text(addr);
    };
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    proxy_on_host_port_text(host, port)
}

/// The menu items in order (replaces the former order): the status item (inactive)
/// → "Open Mesh Town…"/"Join the Mesh Town network…" → separator → the "Exit via…" submenu → the proxy item
/// (inactive) → separator → the actions block (`ActionsUi`: "Invite a member", the checkbox "Allow sites to open via this computer") → separator → the node item → separator → "Quit". The text/availability
/// of the status and proxy items come from `view`/`route`/`proxy`; the submenu has its own `ExitUi::apply_exit_menu`;
/// "Quit" is always enabled.
pub(super) fn build_menu(
    view: crate::gui::tray::app::view::View,
    route: &crate::gui::tray::app::view::RouteLine,
    proxy: &ProxyState,
    exit_submenu: &tray_icon::menu::Submenu,
    actions: &mut ActionsUi,
) -> Result<
    (
        tray_icon::menu::Menu,
        tray_icon::menu::MenuItem,
        tray_icon::menu::MenuItem,
        tray_icon::menu::MenuItem,
        tray_icon::menu::MenuItem,
    ),
    String,
> {
    use crate::gui::tray::app::view::{
        node_item_text, node_status_menu_line, page_enabled, page_text, start_enabled, stop_enabled,
    };
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let status_item =
        MenuItem::with_id(MENU_STATUS, node_status_menu_line(view, route), false, None);
    let open_item = MenuItem::with_id(MENU_OPEN, page_text(view), page_enabled(view.page), None);
    let proxy_item = MenuItem::with_id(MENU_PROXY, proxy_item_text(proxy), false, None);
    let node_item = MenuItem::with_id(
        MENU_NODE,
        node_item_text(view.node),
        start_enabled(view.node) || stop_enabled(view.node),
        None,
    );
    let quit_item = MenuItem::with_id(MENU_QUIT, quit_text(), true, None);

    let menu = Menu::new();
    menu.append(&status_item).map_err(|e| e.to_string())?;
    menu.append(&open_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(exit_submenu).map_err(|e| e.to_string())?;
    menu.append(&proxy_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    actions.append_to(&menu)?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&node_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&quit_item).map_err(|e| e.to_string())?;
    Ok((menu, status_item, open_item, node_item, proxy_item))
}

/// Installs the menu click handler: sends `TrayCmd::Menu` through the same `CmdTx`.
pub(super) fn install_menu_handler(tx: CmdTx) {
    tray_icon::menu::MenuEvent::set_event_handler(Some(
        move |event: tray_icon::menu::MenuEvent| {
            tx.send(TrayCmd::Menu(event.id.0));
        },
    ));
}

/// The raised icon and its parts: the icon itself, the menu items that change text/availability on every
/// `TrayCmd::Update`/`TrayCmd::ExitMenu` (status, "Open Mesh Town…", node, proxy), the icon cache by
/// color (the index is `TrayColor as usize`), the built spinner frames, the key of the installed picture, the exits
/// submenu.
pub(super) struct TrayUi {
    pub(super) tray_icon_handle: tray_icon::TrayIcon,
    pub(super) status_item: tray_icon::menu::MenuItem,
    pub(super) open_item: tray_icon::menu::MenuItem,
    pub(super) node_item: tray_icon::menu::MenuItem,
    pub(super) proxy_item: tray_icon::menu::MenuItem,
    pub(super) icons: [tray_icon::Icon; 4],
    /// Spinner frames built so far (see `FrameCache`).
    pub(super) frames: FrameCache<tray_icon::Icon>,
    /// The key of the picture currently in the tray (`icon_picture`): the color and what is drawn on top of it.
    pub(super) shown_icon_key: IconPicture,
    pub(super) exit_submenu: super::exit_menu::ExitUi,
    pub(super) actions: ActionsUi,
    pub(super) vote: super::vote_item::VoteUi,
    pub(super) nets: super::nets_ui::NetsUi,
    pub(super) extras: super::extras_ui::ExtrasUi,
}

impl TrayUi {
    /// Sets the view's icon only if its key (picture) changed: the same picture is not
    /// reinstalled. A raster or tray error — the icon stays as it was, silently, the key stays as it was (next
    /// time we try again).
    pub(super) fn show_icon(&mut self, key: IconPicture) {
        if key == self.shown_icon_key {
            return;
        }
        self.shown_icon_key = self.install(key).map_or(self.shown_icon_key, |()| key);
    }

    /// Builds the view's icon and puts it into the tray; a raster or tray error gives `Err`, the icon stays as it was.
    fn install(&mut self, picture: IconPicture) -> Result<(), String> {
        let icons = &self.icons;
        let icon = self.frames.get_or_build(picture, |p| view_icon(icons, p))?;
        self.tray_icon_handle
            .set_icon(Some(icon))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "menu_build_tests.rs"]
mod tests;
