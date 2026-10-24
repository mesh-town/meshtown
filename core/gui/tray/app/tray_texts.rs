//! Text table for the tray's user-facing screen: menu labels, status lines, hints and the texts of the
//! macOS dialog. Only human-readable text lives here — log and error lines stay next to the code.
//! A fixed string is a constant (`*_TEXT`; the older names `REASON_*`, `*_LINE`, `EMPTY_HINT`,
//! `SUBMENU_TITLE`, `DISABLED_HINT` and the function `quit_text()` are kept because tests read them), a
//! string with substitutions is a function `*_text(..)` with the same `format!`. There is no logic here:
//! only the wording of this table changes when the text is edited.

use crate::node::output::{APP_NAME, NET_NAME};

// ---- Node and invite items ----

/// Text of the "Start Mesh Town" menu item.
pub(crate) const START_TEXT: &str = "Start Mesh Town";
/// Text of the "Stop Mesh Town" menu item.
pub(crate) const STOP_TEXT: &str = "Stop Mesh Town";
/// Text of the node item while the node is stopping.
pub(crate) const NODE_ITEM_STOPPING_TEXT: &str = "Stopping…";
/// Label of the invite item at rest.
pub(crate) const INVITE_TEXT: &str = "Invite a member";
/// Label of the invite item after the invite was copied to the clipboard (shown for ten seconds).
pub(crate) const INVITE_COPIED_TEXT: &str = "Invite copied ✓";
/// Label of the checkbox "Allow sites to open via this computer".
pub(crate) const SERVE_TEXT: &str = "Let members open sites via this computer";
/// Text of the quit menu item: an action, not the app name.
pub(crate) fn quit_text() -> String {
    "Quit Mesh Town".to_string()
}

// ---- Reasons for an invite or checkbox failure ----

/// The node answered 409: it is still joining the network.
pub(crate) const REASON_JOINING: &str = "Mesh Town is still joining the network";
/// The node answered 409 `not_admin`: only admins of the network issue invites.
pub(crate) const REASON_NOT_ADMIN: &str = "only admins can invite to this network";
/// Other node answers and network failures.
pub(crate) const REASON_NO_ANSWER: &str = "Mesh Town didn't create an invite";
/// No clipboard program in the system.
pub(crate) const REASON_NO_TOOL: &str = "install wl-copy, xclip or xsel";
/// A clipboard program exists but did not accept the string.
pub(crate) const REASON_CLIP_FAILED: &str = "the clipboard didn't accept the text";
/// The node did not accept the `exit.enabled` change after two rounds.
pub(crate) const REASON_SERVE_REFUSED: &str = "Mesh Town didn't accept the change";

// ---- First menu line: invite and checkbox notes ----

/// First menu line after a successful copy: the invite is single-use, it must not go to a group chat.
pub(crate) const INVITE_COPIED_LINE: &str =
    "Invite copied — single use, valid for 24 h; send it privately";
/// First menu line when only the issuing computer can let the invite in.
pub(crate) const INVITE_COPIED_ISSUER_ONLY_LINE: &str =
    "Invite copied — only this computer can let them in; keep it on";
/// First menu line when the invite can be accepted by the issuer and one more node.
pub(crate) const INVITE_COPIED_ISSUER_AND_ONE_LINE: &str =
    "Invite copied — keep this computer on until they join";

/// System notification title after copying an invite that depends on this computer; the product is named
/// because some systems label the notification with another sender.
pub(crate) const INVITE_NOTICE_TITLE: &str = "Mesh Town — keep this computer on";
/// Notification body when only the issuing computer can let the invite in.
pub(crate) const INVITE_NOTICE_ISSUER_ONLY_BODY: &str =
    "Invite copied. Only this computer can let them in. Keep it on until they join.";
/// Notification body when the issuer and one more member can let the invite in.
pub(crate) const INVITE_NOTICE_ISSUER_AND_ONE_BODY: &str =
    "Invite copied. Only this computer and one member can let them in. Keep it on until they join.";
/// Label of the item and first menu line when the invite could not be copied.
pub(crate) fn invite_copy_failed_text(reason: &str) -> String {
    format!("Couldn't copy the invite: {reason}")
}
/// First menu line when the "Allow sites to open" checkbox could not be turned on.
pub(crate) fn serve_enable_failed_text(reason: &str) -> String {
    format!("Couldn't turn on sharing: {reason}")
}
/// First menu line when the "Allow sites to open" checkbox could not be turned off.
pub(crate) fn serve_disable_failed_text(reason: &str) -> String {
    format!("Couldn't turn off sharing: {reason}")
}

// ---- Menu status line (the item is inactive) ----

/// There is no node: the browser proxy will not open anything.
pub(crate) fn status_absent_text() -> String {
    format!("{NET_NAME} is stopped — sites won't open through it")
}
/// Our own node was banned from the network by a vote: nothing opens through it.
pub(crate) fn status_excluded_text() -> String {
    format!("This computer is no longer in the network — sites won't open through {NET_NAME}")
}
/// The menu item shown while a ban vote is in progress; a click opens the members section.
pub(crate) const VOTE_ITEM_TEXT: &str = "Ban vote in progress — open Members…";
/// The tail of the icon hint while a ban vote is in progress.
pub(crate) const VOTE_TOOLTIP_TAIL: &str = " · a ban vote is in progress";
/// The menu item shown while a node waits to join; a click opens the members section.
pub(crate) const ADMIT_ITEM_TEXT: &str = "A node is waiting to join — open Members…";
/// The tail of the icon hint while a node waits to join.
pub(crate) const ADMIT_TOOLTIP_TAIL: &str = " · a node is waiting to join";
/// The tail of the icon hint while this computer waits for members to confirm it.
pub(crate) const PENDING_TOOLTIP_TAIL: &str = " · waiting for members to confirm this computer";
/// The node is starting.
pub(crate) fn status_starting_text() -> String {
    format!("{NET_NAME} is starting…")
}
/// The node is stopping.
pub(crate) const STATUS_STOPPING_TEXT: &str = "Mesh Town is stopping…";
/// The node is not responding (the page is unavailable, or the node is unknown without another reason).
pub(crate) fn status_unresponsive_text() -> String {
    format!("{NET_NAME} is not responding")
}
/// The node page port is in use by another program.
pub(crate) fn status_page_port_busy_text(port: u16) -> String {
    format!("{NET_NAME} isn't running: another program uses page port {port}")
}
/// The node page is turned off in the settings file, so the icon cannot see the node.
pub(crate) const STATUS_PAGE_DISABLED_TEXT: &str =
    "The icon can't reach Mesh Town: page is off in settings file";
/// The "as usual" exit is selected.
pub(crate) fn route_direct_text() -> String {
    "In the browser, sites open directly".to_owned()
}
/// One exit is selected and it is online.
pub(crate) fn route_via_text(name: &str) -> String {
    format!("In the browser, sites open via {name}")
}
/// The selected exit is offline; `mark` is the sign before the name.
pub(crate) fn route_exit_offline_text(mark: &str, name: &str) -> String {
    format!("{mark} {name} is offline — sites won't open until it's back")
}
/// Several exits are selected and none is online.
pub(crate) const ROUTE_ALL_OFFLINE_TEXT: &str = "Chosen members are offline — sites won't open";
/// Grey icon: not joined to a network yet.
pub(crate) fn net_none_status_text() -> String {
    format!("You haven't joined a {NET_NAME} network yet")
}
/// Grey icon: connecting to the network.
pub(crate) fn net_connecting_status_text() -> String {
    format!("Connecting to the {NET_NAME} network…")
}
/// Grey icon: no connection to the network.
pub(crate) fn net_lost_status_text() -> String {
    format!("No connection to the {NET_NAME} network")
}

// ---- The "Open" item and its hints ----

/// The "Open" item when the node page is ready.
pub(crate) fn page_open_text() -> String {
    format!("Open {APP_NAME}…")
}
/// The item that opens the form for adding one more network.
pub(crate) const ADD_NET_TEXT: &str = "Add a network…";
/// The flat-menu invite item for a member who may not invite.
pub(crate) const INVITE_ADMINS_ONLY_TEXT: &str = "Only admins can invite";
/// The "Open" item while not yet joined to a network.
pub(crate) fn page_join_text() -> String {
    "Join or create a network…".to_string()
}
/// The "Open" item while the node page is starting.
pub(crate) const PAGE_STARTING_TEXT: &str = "Mesh Town page is starting…";
/// The "Open" item when the node page is unavailable.
pub(crate) const PAGE_UNAVAILABLE_TEXT: &str = "Mesh Town page is unavailable";
/// Second (grey) menu item when the node page is turned off.
pub(crate) const DISABLED_HINT: &str =
    "Turn the page on in the settings file (web.enabled), then restart";
/// Second (grey) menu item when the page port is in use.
pub(crate) fn port_busy_hint_text() -> String {
    format!("Close that program — \"{START_TEXT}\" will become available")
}

// ---- Why the node did not start ----

/// The proxy port is in use by another program right now.
pub(crate) fn start_proxy_busy_text(port: u16) -> String {
    format!("{NET_NAME} isn't running: another program uses proxy port {port}")
}
/// According to the node log, the proxy port was in use at start.
pub(crate) fn start_proxy_was_busy_text(port: u16) -> String {
    format!("{NET_NAME} isn't running: proxy port {port} was in use")
}
/// The node stopped with a known exit code.
pub(crate) fn start_failed_code_text(code: i32) -> String {
    format!("{NET_NAME} stopped with an error (code {code})")
}
/// The node stopped with an error and there is no code.
pub(crate) fn start_failed_text() -> String {
    format!("{NET_NAME} stopped with an error")
}
/// The node stopped; the reason is a line from the node log.
pub(crate) fn start_failed_with_text(reason: &str) -> String {
    format!("{NET_NAME} stopped: {reason}")
}
/// Hint for a proxy port that is in use right now.
pub(crate) fn start_proxy_busy_hint_text() -> String {
    format!("Close that program, then click \"{START_TEXT}\"")
}
/// Hint for a proxy port that was in use at start.
pub(crate) fn start_proxy_was_busy_hint_text() -> String {
    format!("Click \"{START_TEXT}\" again")
}
/// Hint: details are in the node log next to the settings file.
pub(crate) fn node_log_hint_text(log_name: &str) -> String {
    format!("Details: {log_name}, next to the settings file")
}

// ---- "Exit via…" submenu ----

/// Title of the tray submenu.
pub(crate) const SUBMENU_TITLE: &str = "Exit via…";
/// Hint of the empty submenu: no member offers itself as an exit.
pub(crate) const EMPTY_HINT: &str = "No member lets sites open via their computer yet";
/// Text of the first submenu item — sites open as usual.
pub(crate) fn direct_text() -> String {
    "Directly, without an exit".to_owned()
}
/// Label of a member without a name: "member <short id>".
pub(crate) fn peer_unnamed_text(id: &str) -> String {
    format!("Member {id}")
}
/// Label of a member that is offline.
pub(crate) fn peer_offline_text(base: &str) -> String {
    format!("{base} (offline)")
}

// ---- Proxy item ----

/// The proxy is turned off in the settings file.
pub(crate) const PROXY_OFF_SETTINGS_TEXT: &str = "Proxy is off in the settings file";
/// The proxy is turned off by a flag for this run.
pub(crate) const PROXY_OFF_FLAG_TEXT: &str = "Proxy is off until Mesh Town restarts";
/// The proxy is on and the address did not split into host and port.
pub(crate) fn proxy_on_addr_text(addr: &str) -> String {
    format!("Browser proxy: {addr} (SOCKS5 or HTTP)")
}
/// The proxy is on and the host and port are known.
pub(crate) fn proxy_on_host_port_text(host: &str, port: &str) -> String {
    format!("Browser proxy: {host}, port {port} (SOCKS5 or HTTP, no password)")
}

// ---- macOS dialog "icon is hidden" ----

/// Dialog title: the icon is hidden in the menu bar.
#[cfg(target_os = "macos")]
pub fn hidden_icon_title_text() -> String {
    format!("The {APP_NAME} icon is hidden in the menu bar")
}
/// Dialog explanation: which switch to turn on in System Settings.
#[cfg(target_os = "macos")]
pub fn hidden_icon_hint_text() -> String {
    format!("Turn on {APP_NAME} in System Settings: Menu Bar → Allow in the Menu Bar.")
}
/// First dialog button: open System Settings.
#[cfg(target_os = "macos")]
pub const HIDDEN_ICON_OPEN_SETTINGS_TEXT: &str = "Open System Settings";
/// Second dialog button: close without going there.
#[cfg(target_os = "macos")]
pub const HIDDEN_ICON_LATER_TEXT: &str = "Not now";
