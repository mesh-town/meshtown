//! Starting the `tray` subcommand: checking the runtime and graphics, reading the config,
//! the child's arguments, building the tray channel and handing it to the poller (`poller::poll_node`), then the
//! tray loop `tray::run`. This is also where the page link (`page_url`) and reading `web-token` from a file live.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::OnceLock;

use clap::{Args as _, FromArgMatches as _};

use crate::cli::args::toggle_from_flag_pair;
use crate::cli::commands::RunArgs;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::quit::{exit_now, QuitSignal};
use crate::gui::tray::{NodeCmd, TrayCmd};

use crate::config::config_file::ListenAccess;

use super::poll_types::{PollEnv, PollTiming};
use super::poller::poll_node;
use super::start_fail::{busy_proxy_port, StartFail};

#[derive(clap::Args)]
pub struct TrayArgs {
    /// Settings file (as for run); can be set with the MESHTOWN_CONFIG variable.
    #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
    config: Option<PathBuf>,
    /// Port of the node page on 127.0.0.1 (default 7777).
    // 0 in `run` means a random port, and the tray would not find it: only 1..=65535.
    #[arg(long, value_name = "PORT", value_parser = clap::value_parser!(u16).range(1..=65535))]
    web_port: Option<u16>,
    /// Node options, as for meshtown run; written after --.
    #[arg(last = true, value_name = "RUN_OPTIONS")]
    run_args: Vec<OsString>,
}

#[path = "run_env.rs"]
mod env;
use env::{build_poll_env, spawn_poller};

/// Node port and the path to `web-token` — for `page_url()` (set once in `run_tray`).
static PAGE_PORT_AND_TOKEN_PATH: OnceLock<(u16, PathBuf)> = OnceLock::new();

pub fn run_tray(args: TrayArgs, tray: &'static dyn crate::gui::tray::TrayBackend) -> ! {
    preflight();
    let (settings, token_path) = read_config_or_exit(args.config.as_deref());
    let port = args.web_port.unwrap_or(settings.process.web_port);
    let _ = PAGE_PORT_AND_TOKEN_PATH.set((port, token_path.clone()));

    let quit = QuitSignal::new();
    let (node_tx, node_rx) = tokio::sync::mpsc::unbounded_channel::<NodeCmd>();
    let (tx, core) = crate::gui::tray::channel(quit.clone(), page_url, node_tx, tray);
    let send = tx.clone();
    let env = build_poll_env(
        (port, token_path),
        (&settings, &args),
        node_rx,
        quit,
        move |cmd| send.send(cmd),
        tray,
    );
    spawn_poller(env, move |cmd| tx.send(cmd));
    crate::gui::tray::run(core)
}

/// A headless environment: without a multi-threaded runtime or graphics the tray does not start.
fn preflight() {
    if !is_multi_thread_runtime(&tokio::runtime::Handle::current()) {
        say("a multi-threaded tokio runtime is required");
        exit_now(1);
    }
    if let Some(why) = crate::gui::tray::app::mode::headless_reason() {
        say(format!("no tray icon: {why}"));
        exit_now(1);
    }
}

/// A `[tray] …` line in the process log.
fn say(line: impl std::fmt::Display) {
    crate::node::output::say("tray", line);
}

/// On a single-threaded runtime the poller would stall together with the main thread (the tray loop).
fn is_multi_thread_runtime(handle: &tokio::runtime::Handle) -> bool {
    handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
}

/// The config is only read, never written. Returns the settings and the path to
/// `web-token` next to the config.
fn read_config_or_exit(explicit: Option<&Path>) -> (crate::config::config_file::Settings, PathBuf) {
    use crate::config::config_file::{self as cf, ConfigSource, LoadedConfigFile, Settings};
    let env = cf::current_env();
    let source = cf::resolve_config_source(explicit, &env.as_path_env());
    match source {
        ConfigSource::File { path, .. } => {
            let settings = match cf::load_config_file(&path) {
                LoadedConfigFile::Absent => Settings::default(),
                // `field_errors` are not printed — the node will print them.
                LoadedConfigFile::Parsed(p) => p.settings,
                LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
                    say(format!("settings file {}: {why}", path.display()));
                    exit_now(1);
                }
            };
            (settings, token_path_for(&path))
        }
        ConfigSource::Nowhere(why) => {
            say(format!("no place for a settings file: {why}"));
            exit_now(1);
        }
        // `resolve_config_source` does not return it (the tray has no `--no-config`) — treat it as `Absent`.
        ConfigSource::Disabled => (
            Settings::default(),
            token_path_for(Path::new(cf::FILE_NAME)),
        ),
    }
}

/// `<config directory>/web-token`; an empty parent means `.` (as in `WebOptions::for_run`).
fn token_path_for(config: &Path) -> PathBuf {
    let dir = match config.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    dir.join(crate::web_gui::TOKEN_FILE_NAME)
}

/// The token from the file right now: `trim`med; no file or an error gives an empty string. Needed by `page_url()` here and
/// by the poller's requests (`api::fetch_reply`) — hence `pub(super)`.
pub(super) fn read_token(path: &Path) -> String {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// The tray's `LinkFn`: the page link from the port and the token in the file at the moment of the click.
fn page_url() -> Option<String> {
    let (port, path) = PAGE_PORT_AND_TOKEN_PATH.get()?;
    page_url_for(*port, path)
}

fn page_url_for(port: u16, token_path: &Path) -> Option<String> {
    let token = read_token(token_path);
    if token.is_empty() {
        None
    } else {
        Some(format!("http://127.0.0.1:{port}/?token={token}"))
    }
}

/// The child's arguments: `run`, `--config`, `--web-port`, then everything after `--` as is.
fn child_args(args: &TrayArgs) -> Vec<OsString> {
    let mut out = vec![OsString::from("run")];
    if let Some(config) = &args.config {
        out.push("--config".into());
        out.push(config.clone().into_os_string());
    }
    if let Some(port) = args.web_port {
        out.push("--web-port".into());
        out.push(port.to_string().into());
    }
    out.extend(args.run_args.iter().cloned());
    out
}

/// The proxy port the node will start with; `Err` means the proxy is off, with a reason: the `--no-socks` flag
/// (`ProxyOff::NoSocksFlag`) or the settings file (`ProxyOff::DisabledInSettings`). `--socks-port`, `--socks` and
/// `--no-socks` from the parameters after `--` are parsed once by the same `RunArgs` as in `run`, and a flag
/// beats the settings file (`--socks-port` alone does not turn the proxy on). If parsing fails (the node will say
/// why) or there is no flag, the port and the on/off state come from the file.
fn proxy_plan_for(
    settings_port: u16,
    settings_enabled: bool,
    run_args: &[OsString],
) -> Result<u16, ProxyOff> {
    let run = parse_run_args(run_args);
    let port = run
        .as_ref()
        .and_then(|r| r.socks_port)
        .unwrap_or(settings_port);
    let enabled = run
        .as_ref()
        .and_then(|r| toggle_from_flag_pair(r.socks, r.no_socks));
    proxy_enabled_or_reason(enabled, settings_enabled).map(|()| port)
}

/// The proxy access mode the node will start with: `--socks-access` after `--` beats the settings
/// file; if parsing fails, it comes from the file.
fn proxy_access_for(settings_access: ListenAccess, run_args: &[OsString]) -> ListenAccess {
    parse_run_args(run_args)
        .and_then(|r| r.socks_access)
        .unwrap_or(settings_access)
}

/// Whether the proxy is on: a flag (`Some`) beats the file; off comes with a reason.
fn proxy_enabled_or_reason(flag: Option<bool>, settings: bool) -> Result<(), ProxyOff> {
    match flag {
        Some(true) => Ok(()),
        Some(false) => Err(ProxyOff::NoSocksFlag),
        None if settings => Ok(()),
        None => Err(ProxyOff::DisabledInSettings),
    }
}

/// The parameters after `--` the way `run` will parse them; `None` means parsing failed.
fn parse_run_args(run_args: &[OsString]) -> Option<RunArgs> {
    let argv = std::iter::once(OsString::from("run")).chain(run_args.iter().cloned());
    RunArgs::augment_args(clap::Command::new("run"))
        .try_get_matches_from(argv)
        .ok()
        .and_then(|m| RunArgs::from_arg_matches(&m).ok())
}

/// Real child launch: the same binary, stdin is null; no groups and no detaching.
fn spawn_child(argv: &[OsString]) -> std::io::Result<Child> {
    #[cfg_attr(not(any(windows, target_os = "macos")), allow(unused_mut))]
    let mut cmd = child_command(argv)?;
    #[cfg(any(windows, target_os = "macos"))]
    attach_node_log(&mut cmd);
    cmd.spawn()
}

/// The command that starts this binary with the given arguments and a null stdin.
fn child_command(argv: &[OsString]) -> std::io::Result<std::process::Command> {
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    cmd.args(argv).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    Ok(cmd)
}

/// Sends the child's output to the node log when it needs one; without the log, the output is dropped.
#[cfg(any(windows, target_os = "macos"))]
fn attach_node_log(cmd: &mut std::process::Command) {
    if !crate::cli::os_console::child_needs_node_log() {
        return;
    }
    match node_log_pair() {
        Ok((out, err)) => {
            cmd.stdout(std::process::Stdio::from(out))
                .stderr(std::process::Stdio::from(err));
        }
        Err(e) => {
            say(format!("couldn't open the node log: {e}"));
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }
    }
}

/// Two handles to the node log: one for stdout, one for stderr.
#[cfg(any(windows, target_os = "macos"))]
fn node_log_pair() -> std::io::Result<(std::fs::File, std::fs::File)> {
    let out = crate::cli::os_console::open_node_log()?;
    let err = out.try_clone()?;
    Ok((out, err))
}

/// SIGINT/SIGTERM (Unix), Ctrl-C (Windows); SIGHUP is not caught.
async fn wait_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = ctrl_c() => {}
                    got = term.recv() => {
                        if got.is_none() {
                            std::future::pending::<()>().await;
                        }
                    }
                }
            }
            Err(_) => ctrl_c().await,
        }
    }
    #[cfg(not(unix))]
    ctrl_c().await;
}

/// Ctrl-C; if the handler failed to install, it never fires (rather than firing "immediately").
async fn ctrl_c() {
    if tokio::signal::ctrl_c().await.is_err() {
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
