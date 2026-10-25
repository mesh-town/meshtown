//! Text file with the DHT nodes of a node: `townjump-nodes 1`, `saved <unix seconds>`, then one
//! `<ipv4>:<port>` per line, freshest first. The same format is written by the townjump CLI.

use crate::config::config_file::write_tmp_and_rename;
use crate::node::output::say;
use portable_atomic::AtomicU64;
use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::time::Duration;

const HEADER: &str = "townjump-nodes 1";

/// Upper bound on nodes read from a file and written to it.
pub(crate) const MAX_NODES: usize = 256;

/// Age after which a saved table is not used.
pub(crate) const TTL: Duration = Duration::from_secs(168 * 3600);

/// Parses file text; the error is the short reason why the file is not usable.
pub(crate) fn parse(
    text: &str,
    now: u64,
    ttl: Duration,
) -> Result<Vec<SocketAddrV4>, &'static str> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEADER) {
        return Err("no header");
    }
    let saved = lines.next().and_then(saved_stamp).ok_or("no stamp")?;
    if now.saturating_sub(saved) > ttl.as_secs() {
        return Err("too old");
    }
    let mut seen = HashSet::new();
    let found = lines
        .filter_map(node_line)
        .filter(|addr| seen.insert(*addr));
    let nodes: Vec<SocketAddrV4> = found.take(MAX_NODES).collect();
    match nodes.is_empty() {
        true => Err("no nodes"),
        false => Ok(nodes),
    }
}

fn saved_stamp(line: &str) -> Option<u64> {
    line.trim().strip_prefix("saved ")?.trim().parse().ok()
}

fn node_line(line: &str) -> Option<SocketAddrV4> {
    let addr = SocketAddrV4::from_str(line.trim()).ok()?;
    (addr.port() != 0).then_some(addr)
}

/// File text for `nodes` saved at `now` (unix seconds).
pub(crate) fn render(nodes: &[SocketAddrV4], now: u64) -> String {
    let mut text = format!("{HEADER}\nsaved {now}\n");
    for node in nodes.iter().take(MAX_NODES) {
        text.push_str(&format!("{node}\n"));
    }
    text
}

/// Reads and parses a file; a missing file is a first start, any other defect is reported once.
pub(crate) fn load(path: &Path, now: u64, ttl: Duration) -> Vec<SocketAddrV4> {
    match std::fs::read_to_string(path) {
        Err(e) => not_read(path, &e),
        Ok(text) => parse(&text, now, ttl).unwrap_or_else(|reason| not_used(path, reason)),
    }
}

fn not_read(path: &Path, e: &std::io::Error) -> Vec<SocketAddrV4> {
    if e.kind() == std::io::ErrorKind::NotFound {
        return Vec::new();
    }
    not_used(path, e)
}

fn not_used(path: &Path, reason: impl std::fmt::Display) -> Vec<SocketAddrV4> {
    say(
        "hyper",
        format!("DHT table {} not used: {reason}", path.display()),
    );
    Vec::new()
}

/// Atomic write: a temporary file next to the target, then `rename`.
pub(crate) fn save(path: &Path, nodes: &[SocketAddrV4], now: u64) -> std::io::Result<()> {
    let tmp = temp_path(path);
    let done = write_tmp_and_rename(&tmp, path, &render(nodes, now));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// `<file name>.<pid>-<seq>.tmp` next to the target: two writers of one process never share it.
fn temp_path(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}-{seq}.tmp", std::process::id()));
    PathBuf::from(name)
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
