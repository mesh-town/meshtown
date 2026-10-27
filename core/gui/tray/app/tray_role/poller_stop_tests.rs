//! Tests for the SIGTERM decision in the "Stop" procedure (`poller_stop.rs`).

use super::signals_first;

#[test]
fn signals_first_terms_starting_node() {
    assert!(signals_first(false, true));
}

#[test]
fn signals_first_skips_running_node() {
    assert!(!signals_first(true, true));
}

#[test]
fn signals_first_skips_windows() {
    assert!(!signals_first(false, false));
}
