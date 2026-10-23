//! Node API paths: the main network lives under `/api/<tail>`, an added one under `/api/net/<slot>/<tail>`.

/// The summary of all networks (`GET /api/overview`).
pub(crate) const OVERVIEW_PATH: &str = "/api/overview";

/// The path of `tail` for the network in `slot`; `None` is the main network.
pub(crate) fn api_path(slot: Option<&str>, tail: &str) -> String {
    match slot {
        None => format!("/api/{tail}"),
        Some(slot) => format!("/api/net/{slot}/{tail}"),
    }
}
