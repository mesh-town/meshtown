//! `run --set <path>=<value>`: every assignment is checked before anything starts; a refusal is a usage error.

use super::config::assign::parse_assignment;
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;
use crate::config::config_file::{FieldSet, SetRefusal};

/// Checks the `--set` values in the order typed.
pub(super) fn set_fields(texts: &[String]) -> anyhow::Result<Vec<FieldSet>> {
    texts.iter().map(|t| one(t)).collect()
}

fn one(text: &str) -> anyhow::Result<FieldSet> {
    parse_assignment(text).map_err(|refusal| {
        anyhow::Error::new(UsageError(format!(
            "--set {text}: {}. See: {} run --help",
            reason(&refusal, text),
            program_name()
        )))
    })
}

fn reason(refusal: &SetRefusal, text: &str) -> String {
    match refusal {
        SetRefusal::Unknown(_) if !text.contains('=') => "expected PATH=VALUE".to_string(),
        SetRefusal::Unknown(path) => format!("no such setting `{path}`"),
        SetRefusal::NetworkKey => format!(
            "the network key is not set this way; use --key, or `{} join`",
            program_name()
        ),
        SetRefusal::ReadOnly("web.port") => "the page port is set with --web-port".to_string(),
        SetRefusal::ReadOnly(path) => format!("`{path}` can't be changed"),
        SetRefusal::BadValue { path, expected } => format!("`{path}` expects {expected}"),
    }
}

#[cfg(test)]
#[path = "run_set_tests.rs"]
mod tests;
