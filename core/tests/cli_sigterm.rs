//! Stopping a real `meshtown run` process on SIGTERM (unix only): the process prints
//! "received stop signal" (to stdout, through `say`) and exits with code 0 within `EXIT_LIMIT`.
//!
//! What it does: starts the binary as in `core/tests/run_first_start.rs` (no settings file, no relay,
//! no checks and no SOCKS, `TOWNJUMP_BOOTSTRAP` pointing at a closed port), waits for the line that the node prints
//! once the start phase is over (the web page address is printed earlier, while the transports are still binding),
//! then sends `kill -TERM <pid>` (there is no `libc` in the dev-dependencies). `EXIT_LIMIT` therefore measures
//! only the stop of a running node, not the rest of the start. A second test sends SIGTERM while the start is
//! still under way (the page address is out, the transports are not) and expects a quick exit that never
//! prints the end-of-start line.
//! What it doesn't do: it doesn't check the tray or SIGINT.
//! Links: `core/node/host/signal.rs` (`spawn_stop_signal`).
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use meshtown_core::members::network_key::NetworkKey;

/// The time from SIGTERM to the process exit.
const EXIT_LIMIT: Duration = Duration::from_secs(5);
/// The time from SIGTERM to the process exit when the signal arrives during the start phase.
const START_STOP_LIMIT: Duration = Duration::from_secs(2);
/// The time from launch to the end of the start phase.
const START_LIMIT: Duration = Duration::from_secs(30);

/// The line the node prints at the end of its start phase.
const READY_LINE: &str = "serving as exit for members";
/// The line the start phase prints once the transports are bound; an interrupted start never prints it.
const TRANSPORTS_LINE: &str = "transports:";
/// The line with the web page address, printed while the transports are still binding.
const PAGE_LINE: &str = "http://";

struct Proc {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
}

impl Proc {
    fn start() -> Proc {
        let key = NetworkKey::generate().unwrap().encode();
        let mut child = Command::new(env!("CARGO_BIN_EXE_meshtown"))
            .args(["run", "--no-config", "--key", &key, "--web-port", "0"])
            .args(["--no-relay", "--no-checks", "--no-socks"])
            .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
            .env_remove("MESHTOWN_KEY")
            .env_remove("MESHTOWN_INVITE")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start meshtown");
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

    fn has_line(&self, needle: &str) -> bool {
        self.seen.iter().any(|l| l.contains(needle))
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

#[test]
fn sigterm_stops_node_with_zero_exit_and_message() {
    // given: a running node: the start phase is over (this line comes after the transports are bound)
    let mut node = Proc::start();
    assert!(
        node.wait_line(READY_LINE, START_LIMIT),
        "the node did not finish starting: {:#?}",
        node.seen
    );

    // when: we send SIGTERM
    node.sigterm();
    let exit = node.wait_exit(EXIT_LIMIT);
    node.drain();

    // then: exit with code 0 in time and a line in stdout
    let (code, took) = exit.unwrap_or_else(|| {
        panic!(
            "did not exit within {EXIT_LIMIT:?}; output: {:#?}",
            node.seen
        )
    });
    eprintln!("stop on SIGTERM: code {code}, {took:?}");
    assert_eq!(code, 0, "exit code; output: {:#?}", node.seen);
    assert!(
        node.seen.iter().any(|l| l.contains("received stop signal")),
        "no stop line: {:#?}",
        node.seen
    );
}

#[test]
fn sigterm_during_start_exits_quickly_with_zero_and_message() {
    // given: a node in the middle of its start: the page address is out, the transports are not
    let mut node = Proc::start();
    assert!(
        node.wait_line(PAGE_LINE, START_LIMIT),
        "no page address: {:#?}",
        node.seen
    );
    assert!(
        !node.has_line(TRANSPORTS_LINE),
        "start ended before SIGTERM: {:#?}",
        node.seen
    );

    // when: we send SIGTERM
    node.sigterm();
    let exit = node.wait_exit(START_STOP_LIMIT);
    node.drain();

    // then: exit with code 0 in time, the stop line is printed, the start was not waited for
    let (code, took) = exit.unwrap_or_else(|| {
        panic!(
            "did not exit within {START_STOP_LIMIT:?}; output: {:#?}",
            node.seen
        )
    });
    eprintln!("stop on SIGTERM during the start: code {code}, {took:?}");
    assert_eq!(code, 0, "exit code; output: {:#?}", node.seen);
    assert!(
        node.has_line("received stop signal"),
        "no stop line: {:#?}",
        node.seen
    );
    assert!(
        !node.has_line(TRANSPORTS_LINE),
        "the process waited for the start to end: {:#?}",
        node.seen
    );
}
