//! Output of command results: a JSON reply on one line and aligned tables.

use serde_json::Value;

/// The node's reply as one line on stdout.
pub(crate) fn print_json(v: &Value) {
    println!("{v}");
}

/// A table on stdout: the head always, columns two spaces apart.
pub(crate) fn print_table(head: &[&str], rows: &[Vec<String>]) {
    for line in table_lines(head, rows) {
        println!("{line}");
    }
}

/// The lines of a table: the width of a column is the widest cell counted in characters, and the last
/// column isn't padded.
fn table_lines(head: &[&str], rows: &[Vec<String>]) -> Vec<String> {
    let head: Vec<String> = head.iter().map(|h| h.to_string()).collect();
    let widths: Vec<usize> = (0..head.len())
        .map(|i| column_width(&head, rows, i))
        .collect();
    let line = |cells: &[String]| join_cells(cells, &widths);
    std::iter::once(line(&head))
        .chain(rows.iter().map(|r| line(r)))
        .collect()
}

fn column_width(head: &[String], rows: &[Vec<String>], column: usize) -> usize {
    let cell = |r: &[String]| r.get(column).map_or(0, |c| c.chars().count());
    rows.iter()
        .map(|r| cell(r))
        .chain([cell(head)])
        .max()
        .unwrap_or(0)
}

fn join_cells(cells: &[String], widths: &[usize]) -> String {
    let padded: Vec<String> = cells
        .iter()
        .zip(widths)
        .map(|(c, &w)| format!("{c:<w$}"))
        .collect();
    padded.join("  ").trim_end().to_string()
}

#[cfg(test)]
#[path = "print_tests.rs"]
mod tests;
