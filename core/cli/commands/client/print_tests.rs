//! Tests of the table layout (`print.rs`).

use super::*;

fn row(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|c| c.to_string()).collect()
}

#[test]
fn print_table_aligns_by_chars() {
    let lines = table_lines(
        &["NAME", "STATE"],
        &[row(&["café", "online"]), row(&["a", "offline"])],
    );
    assert_eq!(
        lines,
        vec!["NAME  STATE", "café  online", "a     offline"]
    );
}

#[test]
fn table_without_rows_prints_the_head() {
    assert_eq!(table_lines(&["A", "B"], &[]), vec!["A  B"]);
}

#[test]
fn last_column_is_not_padded() {
    let lines = table_lines(&["A", "B"], &[row(&["x", ""])]);
    assert_eq!(lines[1], "x");
}
