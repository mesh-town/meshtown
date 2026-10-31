//! Which member `<member>` points at: a name first, then the start of an id. A choice that isn't clear is
//! refused with the candidates, never guessed.

use serde_json::Value;

use super::list::clean;
use crate::cli::program_name::program_name;

const MIN_ID_DIGITS: usize = 4;
const FULL_ID_DIGITS: usize = 64;

/// The full id of the member `wanted` names. `Err` is the text of the refusal.
pub(crate) fn resolve_member(members: &Value, wanted: &str) -> Result<String, String> {
    let wanted = wanted.trim();
    let rows: Vec<&Value> = members.as_array().map_or(Vec::new(), |list| {
        list.iter().filter(|m| full_id(m).is_some()).collect()
    });
    if wanted.is_empty() {
        return Err("the member is empty; give a name or the start of an id".to_string());
    }
    let by_name = match_by(&rows, |m| name_is(m, wanted));
    if !by_name.is_empty() {
        return one(&by_name, &format!("several members are named \"{wanted}\""));
    }
    resolve_by_id(&rows, wanted)
}

fn resolve_by_id(rows: &[&Value], wanted: &str) -> Result<String, String> {
    if !looks_like_id(wanted) {
        return Err(unknown(wanted));
    }
    let lower = wanted.to_lowercase();
    let by_id = match_by(rows, |m| {
        full_id(m).is_some_and(|f| f.to_lowercase().starts_with(&lower))
    });
    match by_id.is_empty() {
        true => Err(unknown(wanted)),
        false => one(
            &by_id,
            &format!("several members have an id starting with {wanted}"),
        ),
    }
}

fn match_by<'a>(rows: &[&'a Value], test: impl Fn(&Value) -> bool) -> Vec<&'a Value> {
    rows.iter().copied().filter(|m| test(m)).collect()
}

/// The id when there is exactly one candidate, otherwise the refusal with all of them.
fn one(found: &[&Value], several: &str) -> Result<String, String> {
    match found {
        [only] => Ok(full_id(only).unwrap_or_default().to_string()),
        _ => Err(format!(
            "{several}:\n{}\nUse the start of an id from `{} member list` instead.",
            found
                .iter()
                .map(|m| candidate(m))
                .collect::<Vec<_>>()
                .join("\n"),
            program_name()
        )),
    }
}

/// `<id> <status> <name>` with a mark on the row of this computer.
fn candidate(m: &Value) -> String {
    let id = m["id"].as_str().or(full_id(m)).unwrap_or("?");
    let status = m["status"].as_str().unwrap_or("-");
    let name = clean(m["name"].as_str().unwrap_or("-"));
    let mark = match m["self"].as_bool() {
        Some(true) => " (this computer)",
        _ => "",
    };
    format!("  {id} {status} {name}{mark}")
}

fn unknown(wanted: &str) -> String {
    let short = wanted.chars().count() < MIN_ID_DIGITS;
    match short {
        true => format!(
            "no member is named \"{wanted}\", and an id needs at least {MIN_ID_DIGITS} hex digits. Run `{} member list`",
            program_name()
        ),
        false => format!(
            "no member matches \"{wanted}\". Run `{} member list`",
            program_name()
        ),
    }
}

fn full_id(m: &Value) -> Option<&str> {
    m["id_full"].as_str()
}

/// Rows without a name, or with an empty one, are never matched by name.
fn name_is(m: &Value, wanted: &str) -> bool {
    let name = m["name"].as_str().unwrap_or("");
    !name.is_empty() && name.to_lowercase() == wanted.to_lowercase()
}

fn looks_like_id(wanted: &str) -> bool {
    let len = wanted.chars().count();
    let hex = wanted.chars().all(|c| c.is_ascii_hexdigit());
    hex && (MIN_ID_DIGITS..=FULL_ID_DIGITS).contains(&len)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
