//! System notifications: running the platform's notification tool and the short-lived worker thread
//! that keeps a slow tool off the poller.

use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;

use super::backend::TrayBackend;

/// A notification to show: a short title and a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) title: &'static str,
    pub(crate) body: &'static str,
}

/// The log reason when the notification program is not installed.
pub const REASON_NOTIFY_NO_TOOL: &str = "notification tool not found";
/// The log reason when the notification program failed to start or exited with an error.
pub const REASON_NOTIFY_FAILED: &str = "notification tool failed";

/// Runs `program` with `args` and `env`, all standard streams closed (no console window on Windows), and
/// waits for it to exit.
pub fn run_notifier(
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<(), &'static str> {
    let status = notifier_command(program, args, env).status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(REASON_NOTIFY_FAILED),
        Err(e) if e.kind() == ErrorKind::NotFound => Err(REASON_NOTIFY_NO_TOOL),
        Err(_) => Err(REASON_NOTIFY_FAILED),
    }
}

fn notifier_command(program: &str, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    cmd
}

/// Shows `notice` from a `tray-notify` thread and returns at once; a refusal goes to the log. `None` when
/// the thread could not be created (also logged).
pub(crate) fn spawn_notify(
    tray: &'static dyn TrayBackend,
    notice: Notice,
) -> Option<JoinHandle<()>> {
    let spawned = std::thread::Builder::new()
        .name("tray-notify".into())
        .spawn(move || {
            if let Err(reason) = tray.notify(notice.title, notice.body) {
                log_refusal(reason);
            }
        });
    spawned
        .map_err(|_| log_refusal("couldn't start the thread"))
        .ok()
}

fn log_refusal(reason: &str) {
    crate::node::output::say("tray", format!("invite: notification not shown — {reason}"));
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
