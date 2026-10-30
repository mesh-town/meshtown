//! The `open` subcommand: asks the running node for a sign-in link (`POST /api/auth/link`), opens it in the browser
//! when that makes sense and always prints the addresses. Addresses can carry a token, so they go only through
//! `println!`, not `output::say` (which writes to the `/api/log` log).

use serde_json::Value;

use super::browser;
use super::client::{LinkError, Target};
use crate::node::output::NET_NAME;

/// The facts that decide whether a browser can be started from this process.
pub(super) struct BrowserEnv {
    pub root: bool,
    pub display: bool,
    pub ssh: bool,
    pub desktop_os: bool,
}

impl BrowserEnv {
    /// Reads the current process environment.
    fn current() -> Self {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        Self {
            root: is_root(),
            display: set("DISPLAY") || set("WAYLAND_DISPLAY"),
            ssh: std::env::var_os("SSH_CONNECTION").is_some(),
            desktop_os: cfg!(any(target_os = "macos", windows)),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "android"))]
fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
fn is_root() -> bool {
    false
}

/// Whether to start a browser; `Err` carries the reason for the user.
pub(super) fn may_start_browser(env: &BrowserEnv) -> Result<(), &'static str> {
    if env.root {
        return Err("running as root");
    }
    if env.ssh {
        return Err("this is an SSH session");
    }
    if !env.desktop_os && !env.display {
        return Err("no graphical display");
    }
    Ok(())
}

/// Entry point from `main`.
pub(crate) fn run_open_command(
    config: Option<std::path::PathBuf>,
    web_port: Option<u16>,
    print_only: bool,
) -> anyhow::Result<()> {
    let target = Target::resolve(config, web_port).map_err(anyhow::Error::msg)?;
    open(&target, print_only).map_err(anyhow::Error::msg)
}

fn open(t: &Target, print_only: bool) -> Result<(), String> {
    match t.call("POST", "/api/auth/link", Some("{}")) {
        Ok((200, body)) => {
            present(&body, print_only);
            Ok(())
        }
        Ok((code, body)) => Err(refusal(t, code, &body)),
        Err(LinkError::NotRunning) => Err(format!(
            "{NET_NAME} isn't running on port {}; start it first, or pass --web-port <PORT>",
            t.port
        )),
        Err(LinkError::Other(e)) => Err(e),
    }
}

fn present(body: &str, print_only: bool) {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let urls: Vec<&str> = json["urls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let note = browser_note(urls.first().copied(), print_only);
    for url in &urls {
        println!("{url}");
    }
    if let Some(note) = note {
        println!("{note}");
    }
    println!("{}", tail(json["password_set"].as_bool().unwrap_or(false)));
}

/// Tries to start the browser; returns a line to print when it was not started.
fn browser_note(first: Option<&str>, print_only: bool) -> Option<String> {
    if print_only {
        return None;
    }
    let reason = match (first, may_start_browser(&BrowserEnv::current())) {
        (None, _) => return None,
        (Some(_), Err(why)) => why.to_string(),
        (Some(url), Ok(())) => match browser::launch(url) {
            Ok(()) => return None,
            Err(e) => format!("the browser did not start: {e}"),
        },
    };
    Some(format!(
        "Browser not started ({reason}); open the link yourself."
    ))
}

fn tail(password_set: bool) -> &'static str {
    match password_set {
        true => "Sign in with the page password.",
        false => "The link works once, for 10 minutes.",
    }
}

fn refusal(t: &Target, code: u16, body: &str) -> String {
    if t.token().is_empty() {
        return format!("{NET_NAME} refused (code {code}): no web-token file next to the settings file {}; a sign-in link only works with the permanent token; if {NET_NAME} runs as a service, run the command with sudo", t.path.display());
    }
    let head: String = body.chars().take(200).collect();
    format!("{NET_NAME} refused the sign-in link (code {code}): {head}")
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
