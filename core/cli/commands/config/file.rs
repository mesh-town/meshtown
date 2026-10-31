//! The settings file as `config get` and `config set` use it: its document, the choice between node and file,
//! and the write of one field.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::cli::args::Globals;
use crate::cli::commands::access::settings_io::{edit_file, route, Route};
use crate::cli::commands::client::Target;
use crate::cli::commands::now_unix;
use crate::cli::exit::NotRunning;
use crate::config::config_file::fields::FIELDS;
use crate::config::config_file::{self, FieldSet, LoadedConfigFile, SaveError, Settings};
use crate::node::output::NET_NAME;

/// Where the command acts. With `--net` only a running node can answer: no node is "not running".
pub(super) fn place(g: &Globals) -> anyhow::Result<Route> {
    match route(g)? {
        Route::File(_) if g.net.is_some() => Err(anyhow::Error::new(NotRunning {
            port: page_port(g)?,
        })),
        found => Ok(found),
    }
}

fn page_port(g: &Globals) -> anyhow::Result<u16> {
    let target = Target::resolve(g.config.clone(), g.web_port).map_err(anyhow::Error::msg)?;
    Ok(target.port)
}

/// The stored settings as the `config` document of the node: `{section: {key: value}}`, no secrets.
pub(super) fn document(s: &Settings) -> Value {
    let mut sections: Map<String, Value> = Map::new();
    for f in FIELDS.iter().filter(|f| !f.redacted) {
        if let Some(v) = f.value.render_for_file(s) {
            let section = sections
                .entry(f.section())
                .or_insert_with(|| Value::Object(Map::new()));
            section[f.key()] = v;
        }
    }
    Value::Object(sections)
}

/// The file as the reply of `GET /api/config` would show it: `{"rev": N, "config": {...}}`.
/// A missing file is the defaults at revision 0 (with a note); a damaged one is an error.
pub(super) fn body(path: &Path) -> anyhow::Result<Value> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => Ok(json!({"rev": p.rev, "config": document(&p.settings)})),
        LoadedConfigFile::Absent => {
            eprintln!("no settings file yet: showing the defaults");
            Ok(json!({"rev": 0, "config": document(&Settings::default())}))
        }
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
            anyhow::bail!("settings file is unusable: {why}")
        }
    }
}

/// Writes one field into the file: a missing file is created, an existing one is edited in place.
pub(super) fn save(path: &Path, set: &FieldSet) -> anyhow::Result<()> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Absent => create(path, set),
        _ => edit_file(path, "settings", |s| set.apply_to(s)),
    }
}

fn create(path: &Path, set: &FieldSet) -> anyhow::Result<()> {
    let fresh = config_file::create_fresh(path, now_unix(), |s| set.apply_to(s))?;
    match fresh.saved {
        Ok(_) => Ok(()),
        Err(SaveError::StaleRev(_)) => {
            anyhow::bail!("settings file was created by someone else while saving: try again")
        }
        Err(SaveError::Io(why)) => anyhow::bail!("settings file was not saved: {why}"),
    }
}

/// The note printed when the file took the change instead of a node.
pub(super) fn saved_note(set: &FieldSet, path: &Path) -> String {
    format!(
        "saved {} to {} ({NET_NAME} isn't running: applies at startup)",
        set.path(),
        path.display()
    )
}
