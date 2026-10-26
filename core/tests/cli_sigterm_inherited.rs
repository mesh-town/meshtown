//! A `meshtown run` process that inherits blocked or ignored stop signals from its parent (a shell wrapper, a
//! service manager) still stops on SIGTERM: it prints "received stop signal" and exits with code 0.
//!
//! What it does: starts the binary like `core/tests/cli_sigterm.rs`, but with SIGTERM blocked in the signal mask
//! (or set to ignored) before `exec`, waits for the end of the start phase, sends SIGTERM.
//! What it doesn't do: no SIGINT, no tray.
//! Links: `core/cli/stop_signals.rs`.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
use meshtown_core::transport::net::NodeSecret;

const EXIT_LIMIT: Duration = Duration::from_secs(5);
const START_LIMIT: Duration = Duration::from_secs(30);
const READY_LINE: &str = "serving as exit for members";

/// How the parent leaves SIGTERM for the child.
#[derive(Clone, Copy)]
enum Inherited {
    Blocked,
    Ignored,
}

struct Proc {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
}

impl Proc {
    fn start(inherited: Inherited) -> Proc {
        let key = managed_key_text();
        let mut command = Command::new(env!("CARGO_BIN_EXE_meshtown"));
        command
            .args(["run", "--no-config", "--key", &key, "--web-port", "0"])
            .args(["--no-checks", "--no-socks"])
            .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
            .env_remove("MESHTOWN_KEY")
            .env_remove("MESHTOWN_INVITE")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: the closure only calls async-signal-safe libc functions.
        unsafe {
            command.pre_exec(move || {
                inherit(inherited);
                Ok(())
            });
        }
        let mut child = command.spawn().expect("start meshtown");
        let (tx, rx) = channel();
        pump(child.stdout.take().unwrap(), tx.clone());
        pump(child.stderr.take().unwrap(), tx);
        Proc {
            child,
            lines: rx,
            seen: Vec::new(),
        }
    }

    /// Waits for a line with `needle`; what has been read accumulates in `seen`.
    fn wait_line(&mut self, needle: &str, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let hit = line.contains(needle);
                    self.seen.push(line);
                    if hit {
                        return true;
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return false,
            }
        }
    }

    fn sigterm(&self) {
        let status = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .expect("start kill");
        assert!(status.success(), "kill -TERM failed: {status:?}");
    }

    /// The exit code and the wait time; `None` — did not exit within `limit`.
    fn wait_exit(&mut self, limit: Duration) -> Option<(i32, Duration)> {
        let start = Instant::now();
        while start.elapsed() < limit {
            if let Ok(Some(st)) = self.child.try_wait() {
                return Some((st.code().unwrap_or(-1), start.elapsed()));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        None
    }

    /// Reads the rest of what the process managed to print.
    fn drain(&mut self) {
        std::thread::sleep(Duration::from_millis(200));
        while let Ok(line) = self.lines.try_recv() {
            self.seen.push(line);
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pump(stream: impl Read + Send + 'static, tx: std::sync::mpsc::Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

fn inherit(inherited: Inherited) {
    // SAFETY: plain libc calls on a zeroed, then initialised, signal set.
    unsafe {
        match inherited {
            Inherited::Ignored => {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
            }
            Inherited::Blocked => {
                let mut set: libc::sigset_t = std::mem::zeroed();
                libc::sigemptyset(&mut set);
                libc::sigaddset(&mut set, libc::SIGTERM);
                libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
            }
        }
    }
}

fn stops_on_sigterm(inherited: Inherited) {
    // given: a running node whose parent left SIGTERM blocked or ignored
    let mut node = Proc::start(inherited);
    assert!(
        node.wait_line(READY_LINE, START_LIMIT),
        "the node did not finish starting: {:#?}",
        node.seen
    );

    // when: we send SIGTERM
    node.sigterm();
    let exit = node.wait_exit(EXIT_LIMIT);
    node.drain();

    // then: it stops with code 0 and says why
    let (code, _) = exit.unwrap_or_else(|| {
        panic!(
            "did not exit within {EXIT_LIMIT:?}; output: {:#?}",
            node.seen
        )
    });
    assert_eq!(code, 0, "exit code; output: {:#?}", node.seen);
    assert!(
        node.seen.iter().any(|l| l.contains("received stop signal")),
        "no stop line: {:#?}",
        node.seen
    );
}

#[test]
fn sigterm_blocked_by_the_parent_still_stops_the_node() {
    stops_on_sigterm(Inherited::Blocked);
}

#[test]
fn sigterm_ignored_by_the_parent_still_stops_the_node() {
    stops_on_sigterm(Inherited::Ignored);
}

/// A Managed network key of a fresh base, created by an arbitrary node.
fn managed_key_text() -> String {
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    managed_key_with_label(None, &base, &root)
}
