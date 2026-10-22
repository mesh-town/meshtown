//! Selections from the table.

use super::super::table::FIELDS;
use super::*;

#[test]
fn read_only_follows_table_order() {
    // given: the table
    // when: the read-only selection
    let got = read_only_paths();
    // then: table order, 8 paths
    let want: Vec<&str> = FIELDS
        .iter()
        .filter(|f| got.contains(&f.field_path))
        .map(|f| f.field_path)
        .collect();
    assert_eq!(got, want);
    assert_eq!(got.len(), 8);
}

#[test]
fn find_unknown_is_none() {
    assert!(field_by_path("no.such").is_none());
    assert!(field_by_path("node").is_none());
    assert_eq!(field_by_path("network.key").map(|f| f.redacted), Some(true));
}

#[test]
fn known_keys_cover_computed() {
    // `network` has a stored key and computed label, masked
    assert_eq!(known_keys("network"), vec!["key", "label", "masked"]);
    assert!(known_keys("zzz").is_empty());
}
