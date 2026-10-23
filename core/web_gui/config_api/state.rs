//! `ConfigState`: reading the config for the page — `page_config_document`, `config_get_body`, the directory write probe,
//! the `stale_rev`/`invalid`/`config_write_failed` responses. Editing fields is in `put.rs`/`save_key.rs`
//! of this same folder, shared constants and small helpers are in `support.rs`.

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::Response;
use serde_json::Value;

use crate::config::config_file::{self, fields, FileReport, RunConfig, Settings, SettingsFile};

use super::support::{field_entry, get_path, json_response, AUTOSTART_REASON, FULL_TUNNEL_REASON};

pub struct ConfigState {
    pub file: Option<Arc<dyn SettingsFile>>, // settings file; None — the node runs without a file
    pub secret: NodeSecret,                // node secret, written to the file on every save
    pub file_settings: Settings, // what is in the file now (by the listener's latest knowledge)
    pub boot_file_settings: Settings, // what was in the file when the node last started
    pub file_report: FileReport, // file state; rev — the file revision
    pub cli_override_paths: Vec<&'static str>, // fields set by launch flags
    pub current_network_key: Option<String>, // network key the node works with now
    pub last_boot_id: Option<String>, // boot_id from the node's last snapshot
}

impl ConfigState {
    pub fn from_run(run_config: &RunConfig) -> ConfigState {
        ConfigState {
            file: run_config.file.clone(),
            secret: run_config.secret.clone(),
            file_settings: run_config.file_settings.clone(),
            boot_file_settings: run_config.file_settings.clone(),
            file_report: run_config.file_report.clone(),
            cli_override_paths: run_config.cli_override_paths.clone(),
            current_network_key: run_config.effective_settings.net.network_key.clone(),
            last_boot_id: None,
        }
    }

    /// `render_config_file` of the document for settings `s`, the key `"config"` — without revision/time (they are not
    /// needed here, the document is used only to compare paths and as a basis for the page).
    pub(super) fn config_doc_of(&self, s: &Settings) -> Value {
        let text = config_file::render_config_file(0, 0, &self.secret, s);
        let mut parsed: Value = serde_json::from_str(&text).expect("render yields JSON");
        parsed
            .as_object_mut()
            .and_then(|m| m.remove("config"))
            .expect("render yields config")
    }

    pub(super) fn page_config_document(&self) -> Value {
        let mut doc = self.config_doc_of(&self.file_settings);
        if let Some(obj) = doc.as_object_mut() {
            obj.insert(
                "network".to_string(),
                network_view(self.current_network_key.as_deref()),
            );
            if let Some(node) = obj.get_mut("node").and_then(Value::as_object_mut) {
                if !node.contains_key("name") {
                    node.insert("name".to_string(), Value::Null);
                }
            }
            for (section, reason) in [
                ("autostart", AUTOSTART_REASON),
                ("full_tunnel", FULL_TUNNEL_REASON),
            ] {
                if let Some(sec) = obj.get_mut(section).and_then(Value::as_object_mut) {
                    sec.insert("available".to_string(), Value::Bool(false));
                    sec.insert("reason".to_string(), Value::String(reason.to_string()));
                }
            }
        }
        doc
    }

    /// Probe of the directory for writing.
    fn probe(&self) -> (bool, Option<String>) {
        let Some(path) = self.file.as_ref().map(|f| f.path()) else {
            return (false, None);
        };
        if self.file_report.overwrite_refused {
            return (false, None);
        }
        let dir = probe_dir(path);
        if let Err(message) = probe_write(&dir) {
            return (false, Some(message));
        }
        if file_is_readonly(path) {
            return (
                false,
                Some(format!(
                    "saving unavailable: file {} is read-only",
                    path.display()
                )),
            );
        }
        (true, None)
    }

    pub(super) fn file_block(&self) -> Value {
        let (writable, probe_message) = self.probe();
        let path_of = |f: &Arc<dyn SettingsFile>| Value::String(f.path().display().to_string());
        let path_value = self.file.as_ref().map_or(Value::Null, path_of);
        let message = self
            .file_report
            .message
            .clone()
            .or(probe_message)
            .map(Value::String)
            .unwrap_or(Value::Null);
        let field_errors: Vec<Value> = self
            .file_report
            .field_errors
            .iter()
            .map(|e| field_entry(&e.field_path, &e.message))
            .collect();
        serde_json::json!({
            "path": path_value,
            "state": self.file_report.state.as_str(),
            "writable": writable,
            "message": message,
            "field_errors": field_errors,
        })
    }

    pub(crate) fn pending_restart_paths(&self) -> Vec<&'static str> {
        let cur = self.config_doc_of(&self.file_settings);
        let boot = self.config_doc_of(&self.boot_file_settings);
        let restart_paths = fields::restart_paths();
        fields::editable_paths()
            .into_iter()
            .filter(|&p| restart_paths.contains(&p) || self.cli_override_paths.contains(&p))
            .filter(|&p| get_path(&cur, p) != get_path(&boot, p))
            .collect()
    }

    pub(crate) fn sync_boot_baseline(&mut self, boot_id: &str) {
        if let Some(last) = &self.last_boot_id {
            if last != boot_id {
                self.boot_file_settings = self.file_settings.clone();
            }
        }
        self.last_boot_id = Some(boot_id.to_string());
    }

    pub(crate) fn config_get_body(&self) -> Value {
        serde_json::json!({
            "rev": self.file_report.rev,
            "config": self.page_config_document(),
            "read_only": fields::read_only_paths(),
            "restart_fields": fields::restart_paths(),
            "restart_pending": self.pending_restart_paths(),
            "cli_overrides": self.cli_override_paths,
            "file": self.file_block(),
        })
    }

    pub(crate) fn network_block(&self) -> Value {
        let Some(key) = &self.current_network_key else {
            return Value::Null;
        };
        let Ok(input) = crate::members::network_key::parse_user_input(key) else {
            return Value::Null;
        };
        serde_json::json!({
            "label": input.label,
            "masked": mask_key(key),
            "id_hex": input.key.subkeys().network_id_hex(),
        })
    }

    pub(crate) fn identity_kind_and_reason(&self) -> (&'static str, Option<&'static str>) {
        if self.file_report.persistent_identity {
            ("persistent", None)
        } else if self.file_report.identity_declined {
            ("temporary", Some("declined"))
        } else if self.file_report.overwrite_refused {
            ("temporary", Some("file_preserved"))
        } else {
            ("temporary", Some("file_problem"))
        }
    }

    pub(super) fn stale_rev_response(&self, rev: u64) -> Response {
        json_response(
            StatusCode::CONFLICT,
            serde_json::json!({
                "error": "stale_rev",
                "message": "settings were changed in another tab or by the node itself",
                "rev": rev,
                "config": self.page_config_document(),
            }),
        )
    }

    pub(super) fn invalid_response(&self, fields: Vec<Value>) -> Response {
        json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::json!({
                "error": "invalid",
                "message": "settings not saved",
                "rev": self.file_report.rev,
                "fields": fields,
            }),
        )
    }

    pub(super) fn config_write_failed(&self, message: &str) -> Response {
        json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({
                "error": "config_write_failed",
                "message": message,
                "file": self.file_block(),
            }),
        )
    }
}

fn probe_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Trial creation of a file in the directory — with one recovery attempt (recreate on
/// `AlreadyExists`, create the directory on `NotFound`), then deletes the trial file.
fn probe_write(dir: &Path) -> Result<(), String> {
    let probe_path = dir.join(format!(".config.json.probe-{}", std::process::id()));
    let mut result = probe_create(&probe_path);
    if let Err(e) = &result {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&probe_path);
                result = probe_create(&probe_path);
            }
            std::io::ErrorKind::NotFound => {
                let _ = std::fs::create_dir_all(dir);
                result = probe_create(&probe_path);
            }
            _ => {}
        }
    }
    match result {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe_path);
            Ok(())
        }
        Err(e) => Err(format!(
            "saving unavailable: directory {} doesn't accept writes: {e}",
            dir.display()
        )),
    }
}

fn probe_create(path: &Path) -> std::io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path).map(|_| ())
}

fn file_is_readonly(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|md| md.permissions().readonly())
        .unwrap_or(false)
}

/// `page_config_document().network`: with no key — empty strings; with a key — the label from `parse_user_input` and
/// the mask of the tail after `--` (or of the whole key if there is no `--`).
fn network_view(current_network_key: Option<&str>) -> Value {
    let Some(key) = current_network_key else {
        return serde_json::json!({"label": "", "masked": ""});
    };
    let label = crate::members::network_key::parse_user_input(key)
        .ok()
        .and_then(|i| i.label)
        .unwrap_or_default();
    serde_json::json!({"label": label, "masked": mask_key(key)})
}

fn mask_key(key: &str) -> String {
    match key.split_once("--") {
        Some((head, tail)) => format!("{head}--{}", "•".repeat(tail.chars().count())),
        None => "•".repeat(key.chars().count()),
    }
}
