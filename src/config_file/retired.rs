//! Retired config keys: the file may still carry them, the program does not read them.
//!
//! What it does: holds the list of eight `transport.<key>` paths, finds them in the file and prepares
//! warning lines for the log (one per key and one for the `--port` flag).
//! What it does not do: it does not check the key values and does not write to the log itself.

use serde_json::Value;

/// The version since which the keys have no effect on the node.
pub(crate) const RETIRED_SINCE: &str = "0.1.38";

/// The retired paths, in the order they land in `ParsedConfigFile.retired_paths`.
pub(crate) const RETIRED_PATHS: [&str; 8] = [
    "transport.udp_port",
    "transport.congestion_control",
    "transport.mtu_upper_bound",
    "transport.stream_receive_window",
    "transport.receive_window",
    "transport.max_concurrent_bidi_streams",
    "transport.loss_packet_threshold",
    "transport.loss_time_threshold",
];

/// Returns whether the path `section.key` is retired.
pub(crate) fn is_retired(section: &str, key: &str) -> bool {
    RETIRED_PATHS
        .iter()
        .any(|p| p.split_once('.') == Some((section, key)))
}

/// Returns the retired keys present in `config` with a non-`null` value, as `config.<path>`.
/// `null` is skipped silently: files of 0.1.37 and older carry all eight keys as `null`.
pub(super) fn collect_retired_paths(cfg: &Value) -> Vec<String> {
    RETIRED_PATHS
        .iter()
        .filter(|path| is_present(cfg, path))
        .map(|path| format!("config.{path}"))
        .collect()
}

fn is_present(cfg: &Value, path: &str) -> bool {
    let (section, key) = path.split_once('.').unwrap_or((path, ""));
    cfg.get(section)
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(key))
        .is_some_and(|value| !value.is_null())
}

/// Returns one log line per retired key, in the same order.
pub(crate) fn retired_key_warnings(retired: &[String]) -> Vec<String> {
    retired
        .iter()
        .map(|path| {
            format!(
                "key {path} has no effect since {RETIRED_SINCE} (iroh transport removed) — skipped and will disappear on the next write"
            )
        })
        .collect()
}

/// Returns a warning about the `--port` flag if it was given.
pub fn port_flag_warning(port: Option<u16>) -> Option<String> {
    port.map(|_| {
        format!("flag --port has no effect since {RETIRED_SINCE} (iroh transport removed) — skipped")
    })
}

#[cfg(test)]
#[path = "retired_tests.rs"]
mod tests;
