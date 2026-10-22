//! Tests of `snapshot_self_name`.

use super::snapshot_self_name;

#[test]
fn snapshot_self_name_prefers_effective_settings() {
    assert_eq!(snapshot_self_name(Some("x"), "y"), "x");
}

#[test]
fn snapshot_self_name_falls_back_to_running_node() {
    assert_eq!(snapshot_self_name(None, "y"), "y");
}
