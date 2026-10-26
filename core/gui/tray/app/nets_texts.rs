//! The words of the nested network menu. The flat menu keeps the texts of `tray_texts`.

/// The most member rows a network submenu lists before the "and N more" line.
pub(crate) const MAX_PEER_ROWS: usize = 8;

/// The inactive header of a network that is being joined.
pub(crate) const JOINING_TITLE: &str = "New network — joining…";
/// The "Invite a member" item of a network submenu.
pub(crate) const NET_INVITE_TEXT: &str = "Invite a member";

/// The submenu header of an online network: `friends — 2 of 3 online, 1 to confirm`.
pub(crate) fn net_title_text(label: &str, online: u32, total: u32, admitting: u32) -> String {
    let base = format!("{label} — {online} of {total} online");
    match admitting {
        0 => base,
        n => format!("{base}, {n} to confirm"),
    }
}

/// The submenu header of a network without a connection.
pub(crate) fn net_offline_title_text(label: &str) -> String {
    format!("{label} — no connection")
}

/// The tail of a header of an administered network: `· managed`, for an admin `· managed, you're an admin`.
pub(crate) fn managed_tail(admin: bool) -> &'static str {
    match admin {
        true => " · managed, you're an admin",
        false => " · managed",
    }
}

/// The grey invite row of a member who may not invite to an administered network.
pub(crate) fn net_invite_admins_only_text(label: &str) -> String {
    format!("Only admins can invite to \"{label}\"")
}

pub(crate) fn invite_copied_text(label: &str) -> String {
    format!("Invite to \"{label}\" copied ✓")
}

pub(crate) fn invite_copied_line(label: &str) -> String {
    format!("Invite to \"{label}\" copied — single use, valid for 24 h; send it privately")
}

pub(crate) fn serve_text(label: &str) -> String {
    format!("Let members of \"{label}\" open sites via this computer")
}

pub(crate) fn peer_online_text(name: &str, country: Option<&str>) -> String {
    format!("{} — online", peer_name(name, country))
}

pub(crate) fn peer_offline_row_text(name: &str, country: Option<&str>) -> String {
    format!("{} — offline", peer_name(name, country))
}

fn peer_name(name: &str, country: Option<&str>) -> String {
    match country {
        Some(c) => format!("{name} · {c}"),
        None => name.to_string(),
    }
}

pub(crate) fn more_peers_text(rest: usize) -> String {
    format!("and {rest} more — open Members…")
}

pub(crate) fn open_members_text(label: &str) -> String {
    format!("Open Members of \"{label}\"…")
}

/// The note about a network an admin deleted; a note without a name says "A network".
pub(crate) fn net_deleted_text(label: &str) -> String {
    match label.trim() {
        "" => "A network was deleted by an admin — open Members…".to_string(),
        name => format!("\"{name}\" was deleted by an admin — open Members…"),
    }
}

pub(crate) fn admit_text(label: &str) -> String {
    format!("A node is waiting to join {label} — open Members…")
}

/// The icon hint tail for a network that lost its connection.
pub(crate) fn no_connection_tail(label: &str) -> String {
    format!(" · {label}: no connection")
}

#[cfg(test)]
#[path = "nets_texts_tests.rs"]
mod tests;
