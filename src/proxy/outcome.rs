//! The outcome of serving one SOCKS5 connection — the raw material for the log summary line.

/// How serving one request ended — the raw material for the summary line.
pub struct ProxyOutcome {
    /// The chosen route in words: `direct`, `via 2f9a1c8d` or `no route`.
    pub route_label: String,
    /// The result: `success`, `refused: <reason>` or `interrupted: <reason>` (the exit was removed from the selection or the client left).
    pub result: String,
    /// Bytes that reached the client (from the site or from a member).
    pub bytes_to_client: u64,
    /// Bytes that went out from the client.
    pub bytes_from_client: u64,
}

impl ProxyOutcome {
    /// The route worked and bytes passed.
    pub fn success(route_label: impl Into<String>, bytes_to_client: u64, bytes_from_client: u64) -> Self {
        Self { route_label: route_label.into(), result: "success".to_string(), bytes_to_client, bytes_from_client }
    }

    /// The route failed or was not found: no bytes passed.
    pub fn refused(route_label: impl Into<String>, reason: impl AsRef<str>) -> Self {
        Self {
            route_label: route_label.into(),
            result: format!("refused: {}", reason.as_ref()),
            bytes_to_client: 0,
            bytes_from_client: 0,
        }
    }

    /// The connection was opened through an exit, but the exit was removed from `route.exits` before the transfer ended
    /// (the reset task's decision) — the `splice` counters are lost together with the interrupted task.
    pub fn interrupted(route_label: impl Into<String>) -> Self {
        Self {
            route_label: route_label.into(),
            result: "interrupted: exit no longer selected".to_string(),
            bytes_to_client: 0,
            bytes_from_client: 0,
        }
    }

    /// The SOCKS client left before a route was found: no reply was written to it, no bytes.
    pub fn abandoned() -> Self {
        Self {
            route_label: "no route".to_string(),
            result: "interrupted: client left before a route was found".to_string(),
            bytes_to_client: 0,
            bytes_from_client: 0,
        }
    }
}

/// Builds the serving summary line. A separate pure function —
/// so the format is checked by a test, not by eye.
pub fn outcome_line(target: &str, outcome: &ProxyOutcome, ms: u128) -> String {
    format!(
        "{target} {}: {}, received {} bytes, sent {} bytes, {ms} ms",
        outcome.route_label, outcome.result, outcome.bytes_to_client, outcome.bytes_from_client
    )
}

#[cfg(test)]
#[path = "outcome_tests.rs"]
mod tests;
