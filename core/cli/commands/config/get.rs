//! `config get [<path>]`: one setting or all of them, from the running node or from the settings file.

use serde_json::Value;

use super::assign::shown;
use super::file::{body, place};
use crate::cli::args::{Global, Globals};
use crate::cli::commands::access::settings_io::Route;
use crate::cli::commands::client::print_json;
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;
use crate::config::config_file::fields::{field_by_path, Field, FIELDS};

pub(super) fn run(path: Option<&str>, g: &Globals) -> anyhow::Result<()> {
    g.allow("config get", &[Global::Net, Global::Json])?;
    let field = path.map(pick).transpose()?;
    let reply = match place(g)? {
        Route::Live(node) => node.call("GET", &node.net_path("config"), None)?,
        Route::File(file) => file_reply(&file, field)?,
    };
    note_override(&reply, field);
    match (g.json, field) {
        (true, _) => print_json(&reply),
        (false, Some(f)) => println!("{}", shown(value_at(&reply, f).unwrap_or(&Value::Null))),
        (false, None) => print_lines(&reply),
    }
    Ok(())
}

/// The field a typed path names; the network key is never shown here.
fn pick(path: &str) -> anyhow::Result<&'static Field> {
    let field = field_by_path(path).ok_or_else(|| {
        anyhow::Error::new(UsageError(format!(
            "no such setting `{path}`. See: {} config get",
            program_name()
        )))
    })?;
    match field.redacted {
        true => anyhow::bail!(
            "the network key is not shown here: use `{} network key`",
            program_name()
        ),
        false => Ok(field),
    }
}

/// The settings file as a reply; a computed value is not in the file at all.
fn file_reply(file: &std::path::Path, field: Option<&Field>) -> anyhow::Result<Value> {
    if let Some(f) = field.filter(|f| !f.value.is_stored_in_settings()) {
        anyhow::bail!("`{}` is known only to a running node", f.field_path)
    }
    body(file)
}

/// The value of one field in the `config` document of a reply.
fn value_at<'a>(reply: &'a Value, field: &Field) -> Option<&'a Value> {
    reply["config"].get(field.section())?.get(field.key())
}

/// The stderr note for a field the running node got from its start-up arguments.
pub(super) fn override_note(reply: &Value, field: Option<&Field>) -> Option<String> {
    let list = reply["cli_overrides"].as_array();
    let typed =
        |f: &Field| list.is_some_and(|l| l.iter().any(|p| p.as_str() == Some(f.field_path)));
    field
        .filter(|f| typed(f))
        .map(|_| "this run uses a value set at startup".to_string())
}

fn note_override(reply: &Value, field: Option<&Field>) {
    if let Some(note) = override_note(reply, field) {
        eprintln!("{note}");
    }
}

/// `path=value` for every setting the reply has, in the order of the field table.
pub(super) fn lines(reply: &Value) -> Vec<String> {
    FIELDS
        .iter()
        .filter(|f| !f.redacted)
        .filter_map(|f| Some(format!("{}={}", f.field_path, shown(value_at(reply, f)?))))
        .collect()
}

fn print_lines(reply: &Value) {
    for line in lines(reply) {
        println!("{line}");
    }
}

#[cfg(test)]
#[path = "get_tests.rs"]
mod tests;
