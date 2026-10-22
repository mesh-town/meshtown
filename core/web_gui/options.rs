//! Page launch settings and outward handles: `WebOptions`, `WebHandle`, `WebShared`.

use std::path::PathBuf;
use std::sync::Arc;

pub const TOKEN_FILE_NAME: &str = "web-token";
pub const DEFAULT_DRAIN: std::time::Duration = std::time::Duration::from_secs(2);

pub struct WebOptions {
    pub enabled: bool,
    pub port: u16,
    pub token_dir: Option<PathBuf>,
    pub fixed_token: Option<String>,
    pub drain: std::time::Duration,
}

impl WebOptions {
    pub fn for_run(run_config: &crate::config::config_file::RunConfig) -> WebOptions {
        WebOptions {
            enabled: run_config.effective_settings.process.web_enabled,
            port: run_config.effective_settings.process.web_port,
            token_dir: run_config
                .file_report
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|dir| {
                    if dir.as_os_str().is_empty() {
                        PathBuf::from(".")
                    } else {
                        dir.to_path_buf()
                    }
                }),
            fixed_token: None,
            drain: DEFAULT_DRAIN,
        }
    }
}

pub struct WebHandle {
    pub shared: Arc<WebShared>,
    pub stop: tokio::sync::watch::Sender<bool>,
    pub task: tokio::task::JoinHandle<()>,
}

pub struct WebShared {
    port: u16,
    token: std::sync::RwLock<String>,
    pub(crate) auth: super::auth::state::AuthState,
}

impl WebShared {
    pub(crate) fn new(port: u16, token: String, auth: super::auth::state::AuthState) -> WebShared {
        WebShared {
            port,
            token: std::sync::RwLock::new(token),
            auth,
        }
    }

    /// Sign-in URLs: no password — with a new temporary token, a password is set — without a token.
    pub fn issue_entry_urls(&self) -> Vec<String> {
        self.auth.issue_entry_urls(self.port)
    }

    /// Stores the new `web.access`; narrowing takes effect immediately, widening after a restart.
    pub(crate) fn set_access(&self, mode: crate::config::config_file::ListenAccess) {
        self.auth.set_access(mode);
    }

    /// Stores the new `web.ifaces`; takes effect immediately.
    pub(crate) fn set_ifaces(&self, filter: crate::config::config_file::IfaceFilter) {
        self.auth.set_ifaces(filter);
    }

    pub(crate) fn set_token(&self, token: String) {
        let mut guard = self.token.write().unwrap_or_else(|e| e.into_inner());
        *guard = token;
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> String {
        self.token.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn open_url(&self) -> String {
        format!("http://127.0.0.1:{}/?token={}", self.port, self.token())
    }
}
