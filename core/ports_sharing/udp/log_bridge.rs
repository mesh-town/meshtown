//! Bridge from the framing crate's log to the node journal.
//!
//! What it does: collects the bytes the crate logger writes, cuts them into lines, strips the crate's
//! `<time> [Holesail] [LEVEL] ` prefix (with its colour codes) and passes each line to the `port` journal.
//! What it does not do: lines carrying `UDP_SESSION_MARK` are dropped — those are our own session errors, which
//! the code around the stream already reports in its own words.

use std::io::Write;

use super::session::UDP_SESSION_MARK;
use crate::node::output::say;

/// Byte sink for the crate logger; a line goes to the journal once its `\n` has arrived.
#[derive(Default)]
pub(crate) struct JournalLines {
    pending: Vec<u8>,
}

impl JournalLines {
    fn drain_lines(&mut self) {
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = self.pending.drain(..=end).collect();
            if let Some(text) = journal_text(&String::from_utf8_lossy(&raw)) {
                say("port", text);
            }
        }
    }
}

impl Write for JournalLines {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(data);
        self.drain_lines();
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The message of one crate log line, or `None` if it is empty or one of our own session errors.
pub(crate) fn journal_text(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.contains(UDP_SESSION_MARK) {
        return None;
    }
    let text = match line.split_once("[Holesail]") {
        Some((_, rest)) => strip_level(rest),
        None => line,
    };
    Some(text.trim()).filter(|t| !t.is_empty())
}

/// What follows the level tag: ` [WARN] msg` or, with colour codes, ` \x1b[33m[WARN]\x1b[0m msg`.
fn strip_level(rest: &str) -> &str {
    match rest.split_once(']') {
        Some((_, tail)) => tail.trim_start_matches("\x1b[0m"),
        None => rest,
    }
}

#[cfg(test)]
#[path = "log_bridge_tests.rs"]
mod log_bridge_tests;
