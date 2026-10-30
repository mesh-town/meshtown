//! Start at login: on tray start, repoints an entry that leads to another copy of the program.

use crate::autostart;

/// Runs once at tray start. A failure is only logged; the tray starts anyway.
pub(super) fn refresh_autostart() {
    let Some(backend) = autostart::installed() else {
        return;
    };
    match autostart::refresh_on_start(backend) {
        Ok(true) => crate::node::output::say(
            "tray",
            "start at login: entry pointed elsewhere, rewritten for this copy",
        ),
        Ok(false) => {}
        Err(e) => crate::node::output::say("tray", format!("start at login: refresh failed: {e}")),
    }
}
