//! Why the node didn't start: the reason type and the tray menu lines. Pure values and
//! strings without muda and without the node: the reason is found by the `tray_role` poller (a check of the proxy port and the tail of
//! `meshtown.log`), shown by the tray `tray::core`. The stop reason is the last
//! `Error: …` line of the node log (the node's text is shown as is, truncated to the menu length); without it —
//! its own lines with the exit code.

use crate::gui::tray::app::tray_texts::{
    node_log_hint_text, start_failed_code_text, start_failed_text, start_failed_with_text,
    start_proxy_busy_hint_text, start_proxy_busy_text, start_proxy_was_busy_hint_text,
    start_proxy_was_busy_text,
};

/// The name of the node log next to the settings file.
pub(crate) const NODE_LOG_NAME: &str = "meshtown.log";

/// The reason the node didn't start (or `Idle` — there is no reason).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StartNote {
    Idle,
    /// A trial check before start: the proxy port is in use by another program right now.
    ProxyBusy(u16),
    /// According to the node log: the port was in use when the node started (it may be free now).
    ProxyWasBusy(u16),
    /// The node exited with an error.
    Failed {
        code: Option<i32>,
        has_node_log: bool,
    },
    /// The node exited with an error, the reason is the text after `Error: ` from the node log.
    FailedWith(String),
}

/// The longest first menu line, in characters. On a 1024×675 screen 73 characters
/// fit into the item entirely, 87 don't (AppKit cuts the line with an ellipsis in the middle and loses the port
/// number). The threshold 66 leaves a margin of about 10 % over the measured 73; the phrases with port 1080 themselves are 61 characters
/// (proxy port) and 60 (page port); with port 54794, 62 and 61.
pub(crate) const LINE_MAX_CHARS: usize = 66;

/// A line of at most `LINE_MAX_CHARS` characters (by `chars`): longer — the first `LINE_MAX_CHARS-1` and "…".
fn fit_line(line: String) -> String {
    if line.chars().count() <= LINE_MAX_CHARS {
        return line;
    }
    let head: String = line.chars().take(LINE_MAX_CHARS - 1).collect();
    format!("{head}…")
}

/// The port from the rest of the line `Error: port {number} for the browser proxy is in use by another program …`.
fn busy_port(rest: &str) -> Option<u16> {
    let tail = rest.strip_prefix("port ")?;
    let (number, after) = tail.split_once(' ')?;
    after
        .starts_with("for the browser proxy is in use by another program")
        .then_some(())?;
    number.parse().ok()
}

/// The rest of the last log line that starts with `Error: `.
fn last_error_rest(log_tail: Option<&str>) -> Option<&str> {
    log_tail?
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("Error: "))
}

/// The reason by the exit code and the node log tail: the last line `Error: ` is taken.
pub(crate) fn start_note_from(code: Option<i32>, log_tail: Option<&str>) -> StartNote {
    let rest = last_error_rest(log_tail);
    if let Some(port) = rest.and_then(busy_port) {
        return StartNote::ProxyWasBusy(port);
    }
    match rest.map(str::trim).filter(|r| !r.is_empty()) {
        Some(reason) => StartNote::FailedWith(reason.to_owned()),
        None => StartNote::Failed {
            code,
            has_node_log: log_tail.is_some(),
        },
    }
}

/// The first menu line by the reason; `None` — there is no reason, the line is the usual one. Lines with a port say
/// whose it is: "proxy port" here, "page port" in `view_unknown.rs` — without this the two menu lines about
/// different ports would read word for word the same. The length is at most 66 characters (test
/// `port_lines_fit_the_menu`): on a 1024×675 screen 73 characters fit, 87 didn't, and
/// AppKit squeezed the item with an ellipsis in the middle, losing the port number. The words "for the browser proxy" are
/// not here — they stay in the node log and on the page.
pub(crate) fn start_note_line(note: &StartNote) -> Option<String> {
    match note {
        StartNote::Idle => None,
        StartNote::ProxyBusy(p) => Some(start_proxy_busy_text(*p)),
        StartNote::ProxyWasBusy(p) => Some(start_proxy_was_busy_text(*p)),
        StartNote::Failed { code: Some(c), .. } => Some(start_failed_code_text(*c)),
        StartNote::Failed { code: None, .. } => Some(start_failed_text()),
        StartNote::FailedWith(r) => Some(fit_line(start_failed_with_text(r))),
    }
}

/// The second (grey) menu item by the reason; `None` — the usual text of the "Open" item.
pub(crate) fn start_note_hint(note: &StartNote) -> Option<String> {
    match note {
        StartNote::Idle => None,
        StartNote::ProxyBusy(_) => Some(start_proxy_busy_hint_text()),
        StartNote::ProxyWasBusy(_) => Some(start_proxy_was_busy_hint_text()),
        StartNote::Failed {
            has_node_log: true, ..
        } => Some(node_log_hint_text(NODE_LOG_NAME)),
        StartNote::Failed {
            has_node_log: false,
            ..
        } => None,
        StartNote::FailedWith(_) => Some(node_log_hint_text(NODE_LOG_NAME)),
    }
}

#[cfg(test)]
#[path = "start_note_tests.rs"]
mod tests;
