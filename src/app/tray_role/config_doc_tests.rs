//! Tests of `ConfigDoc`: reading `rev` and the selected exits from the `GET /api/config` response.

use super::*;
use serde_json::json;

#[test]
fn reads_rev_and_exits() {
    let doc = ConfigDoc::from_value(&json!({
        "rev": 7,
        "config": { "route": { "exits": ["a", "b"] } }
    }));
    assert_eq!(doc.rev, Some(7));
    assert_eq!(doc.exits(), ["a", "b"]);
}

#[test]
fn bad_shapes_fall_back() {
    let doc = ConfigDoc::from_value(&json!({ "rev": "7", "config": 5 }));
    assert_eq!(doc.rev, None);
    assert!(doc.exits().is_empty());
    let doc = ConfigDoc::from_value(&json!({ "config": { "route": { "exits": "a" } } }));
    assert!(doc.exits().is_empty());
}
