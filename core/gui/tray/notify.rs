//! System notifications: running the platform's notification tool and the short-lived worker thread
//! that keeps a slow tool off the poller.

use std::io::ErrorKind;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

/// The longest a notification program may run before it is killed.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(10);
/// The pause between checks whether the notification program has exited.
const NOTIFY_POLL_STEP: Duration = Duration::from_millis(50);

/// Runs `program` with `args` and `env`, all standard streams closed (no console window on Windows), and
/// waits for it to exit, at most `NOTIFY_TIMEOUT`; a program still running then is killed.
pub fn run_notifier(
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<(), &'static str> {
    run_notifier_within(program, args, env, NOTIFY_TIMEOUT)
}

fn run_notifier_within(
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
    limit: Duration,
) -> Result<(), &'static str> {
    let mut child = match notifier_command(program, args, env).spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(REASON_NOTIFY_NO_TOOL),
        Err(_) => return Err(REASON_NOTIFY_FAILED),
    };
    match wait_within(&mut child, limit) {
        Some(status) if status.success() => Ok(()),
        _ => Err(REASON_NOTIFY_FAILED),
    }
}

/// The exit status of `child`, or `None` when it did not exit within `limit` (it is killed and reaped) or
/// could not be waited for.
fn wait_within(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if start.elapsed() < limit => std::thread::sleep(NOTIFY_POLL_STEP),
            _ => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    None
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
