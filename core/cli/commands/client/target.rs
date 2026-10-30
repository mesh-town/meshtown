//! How the console subcommands connect to a running node: where to go (settings file, port,
//! token); the HTTP/1.0 client itself is `web_gui::probe`. A refused connection is named separately
//! (`NotRunning`) — then the command writes to disk itself.

use std::path::PathBuf;
use std::time::Duration;

use crate::config::config_file::{self, ConfigSource, LoadedConfigFile, Settings};
use crate::web_gui::probe;
pub(crate) use crate::web_gui::probe::LinkError;
use crate::web_gui::TOKEN_FILE_NAME;

const TIMEOUT: Duration = Duration::from_secs(5);

/// Where to go: the settings file, its directory (where `web-token` and `web-password` also live) and the page port.
pub(crate) struct Target {
    pub(crate) path: PathBuf,
    pub(crate) dir: PathBuf,
    pub(crate) port: u16,
}

impl Target {
    /// The settings file is the same as for `config-path`; the port is `--web-port`, otherwise `web.port` from the file, otherwise the default.
    pub(crate) fn resolve(
        config: Option<PathBuf>,
        web_port: Option<u16>,
    ) -> Result<Target, String> {
        let env = config_file::current_env();
        let source = config_file::resolve_config_source(config.as_deref(), &env.as_path_env());
        let path = match source {
            ConfigSource::File { path, .. } => path,
            ConfigSource::Nowhere(why) => {
                return Err(format!("no place for a settings file: {why}"))
            }
            ConfigSource::Disabled => {
                return Err("no place for a settings file: the settings file is turned off".into())
            }
        };
        let dir = match path.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let port = web_port.unwrap_or_else(|| file_port(&path));
        Ok(Target { path, dir, port })
    }

    /// The permanent token from `web-token`; if there is no file, it is empty (the node will refuse, not us).
    pub(crate) fn token(&self) -> String {
        let text = std::fs::read_to_string(self.dir.join(TOKEN_FILE_NAME)).unwrap_or_default();
        text.trim().to_string()
    }

    pub(crate) fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), LinkError> {
        call(self.port, &self.token(), method, path, body)
    }
}

fn file_port(path: &std::path::Path) -> u16 {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p.settings.process.web_port,
        _ => Settings::default().process.web_port,
    }
}

/// One request to the node: the response code and the body. `Host` and `Origin` are `127.0.0.1:<port>`, the token is in the header.
fn call(
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), LinkError> {
    probe::call_within(TIMEOUT, port, token, method, path, body)
}
