//! How the `password` and `access` subcommands connect to a running node: where to go (settings file, port,
//! token) and a small HTTP/1.0 client for the node page on 127.0.0.1. It has no TLS, redirects
//! or retries; a refused connection is named separately (`NotRunning`) — then the command writes to disk itself.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use meshtown::config_file::{self, ConfigSource, LoadedConfigFile, Settings};
use meshtown::web::TOKEN_FILE_NAME;

const TIMEOUT: Duration = Duration::from_secs(5);

/// Why the call failed.
#[derive(Debug)]
pub(super) enum LinkError {
    /// The connection was refused: no node is listening on this port.
    NotRunning,
    /// Any other failure; the text is in English.
    Other(String),
}

/// Where to go: the settings file, its directory (where `web-token` and `web-password` also live) and the page port.
pub(super) struct Target {
    pub(super) path: PathBuf,
    pub(super) dir: PathBuf,
    pub(super) port: u16,
}

impl Target {
    /// The settings file is the same as for `config-path`; the port is `--web-port`, otherwise `web.port` from the file, otherwise the default.
    pub(super) fn resolve(config: Option<PathBuf>, web_port: Option<u16>) -> Result<Target, String> {
        let env = config_file::current_env();
        let source = config_file::resolve_config_source(config.as_deref(), &env.as_path_env());
        let path = match source {
            ConfigSource::File { path, .. } => path,
            ConfigSource::Nowhere(why) => return Err(format!("no place for a settings file: {why}")),
            ConfigSource::Disabled => return Err("no place for a settings file: the settings file is turned off".into()),
        };
        let dir = match path.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let port = web_port.unwrap_or_else(|| file_port(&path));
        Ok(Target { path, dir, port })
    }

    /// The permanent token from `web-token`; if there is no file, it is empty (the node will refuse, not us).
    pub(super) fn token(&self) -> String {
        let text = std::fs::read_to_string(self.dir.join(TOKEN_FILE_NAME)).unwrap_or_default();
        text.trim().to_string()
    }

    pub(super) fn call(&self, method: &str, path: &str, body: Option<&str>) -> Result<(u16, String), LinkError> {
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
pub(super) fn call(
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), LinkError> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(connect_error)?;
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let request = request_text(port, token, method, path, body);
    stream.write_all(request.as_bytes()).map_err(link_error_from_io)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(link_error_from_io)?;
    parse_response(&raw)
}

fn connect_error(e: std::io::Error) -> LinkError {
    match e.kind() {
        std::io::ErrorKind::ConnectionRefused => LinkError::NotRunning,
        _ => link_error_from_io(e),
    }
}

fn link_error_from_io(e: std::io::Error) -> LinkError {
    LinkError::Other(format!("connection to the node: {e}"))
}

fn request_text(port: u16, token: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let body = body.unwrap_or("");
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n");
    req.push_str(&format!("Origin: http://127.0.0.1:{port}\r\nX-Meshtown-Token: {token}\r\n"));
    if !body.is_empty() {
        req.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n", body.len()));
    }
    format!("{req}\r\n{body}")
}

fn parse_response(raw: &[u8]) -> Result<(u16, String), LinkError> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let code = head.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok());
    match code {
        Some(code) => Ok((code, body.to_string())),
        None => Err(LinkError::Other("the node did not reply over HTTP".into())),
    }
}

#[cfg(test)]
#[path = "node_link_tests.rs"]
mod tests;
