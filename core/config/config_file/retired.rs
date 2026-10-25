//! The log line about retired fields: keys that older files may still hold, which are ignored and dropped on the next write.

use crate::node::output::say;

/// Says once, in category `config`, which retired fields were found; nothing for an empty list.
/// `scope` is put before the text (empty, or a slot prefix such as `net slot ab12: `); paths are shown without `config.`.
pub fn log_retired(scope: &str, retired_paths: &[String]) {
    if retired_paths.is_empty() {
        return;
    }
    say("config", retired_text(scope, retired_paths));
}

fn retired_text(scope: &str, retired_paths: &[String]) -> String {
    let shown: Vec<&str> = retired_paths
        .iter()
        .map(|p| p.strip_prefix("config.").unwrap_or(p))
        .collect();
    format!(
        "{scope}deprecated fields ignored and dropped on the next write: {}",
        shown.join(", ")
    )
}

#[cfg(test)]
#[path = "retired_tests.rs"]
mod tests;
