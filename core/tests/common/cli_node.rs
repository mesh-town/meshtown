//! A real `meshtown` process for command line tests: `CliNode` runs the node on its own settings file in a
//! temporary directory with a page port chosen by the system, and `cli`/`cli_in` run client commands against it.
//! Each scenario uses only part of this module: `#![allow(dead_code)]` below is about the functions kept for a
//! neighbouring file. The process never sees the environment of the person running the tests: no key, invite or
//! settings path variable, and the home directories point at the temporary directory.
#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How long `CliNode::start` waits for the page address line.
const START_LIMIT: Duration = Duration::from_secs(30);

/// A running node; the process is killed and the directory removed on drop.
pub struct CliNode {
    pub child: Child,
    pub dir: PathBuf,
    pub port: u16,
    lines: Receiver<String>,
    seen: Vec<String>,
}

impl CliNode {
    /// Starts `run --config <dir>/config.json --web-port 0 --no-checks --no-socks` plus `extra`; returns once the
    /// page address is printed.
    pub fn start(extra: &[&str]) -> CliNode {
        CliNode::start_in(scratch_dir("node"), extra)
    }

    /// The same, with `config_json` written as the settings file first.
    pub fn start_with_config(config_json: &str, extra: &[&str]) -> CliNode {
        let dir = scratch_dir("node");
        std::fs::write(dir.join("config.json"), config_json).expect("settings file");
        CliNode::start_in(dir, extra)
    }

    fn start_in(dir: PathBuf, extra: &[&str]) -> CliNode {
        let mut child = isolated(Command::new(env!("CARGO_BIN_EXE_meshtown")), &dir)
            .args(["run", "--config"])
            .arg(dir.join("config.json"))
            .args(["--web-port", "0", "--no-checks", "--no-socks"])
            .args(extra)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start meshtown");
        let (tx, rx) = channel();
        pump(child.stdout.take().unwrap(), tx.clone());
        pump(child.stderr.take().unwrap(), tx);
        let mut node = CliNode {
            child,
            dir,
            port: 0,
            lines: rx,
            seen: Vec::new(),
        };
        let line = node
            .wait_line("?token=", START_LIMIT)
            .expect("the page address was not printed");
        node.port = page_port(&line);
        node
    }

    /// Waits for a line with `needle`; the lines read are accumulated in `seen`.
    pub fn wait_line(&mut self, needle: &str, limit: Duration) -> Option<String> {
        if let Some(old) = self.seen.iter().find(|l| l.contains(needle)) {
            return Some(old.clone());
        }
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    if line.contains(needle) {
                        return Some(line);
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// Waits for the process to exit; `None` if it is still running after `limit`.
    pub fn wait_exit(&mut self, limit: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The permanent token the node wrote beside the settings file.
    pub fn token(&self) -> String {
        std::fs::read_to_string(self.dir.join("web-token"))
            .expect("web-token")
            .trim()
            .to_string()
    }

    /// Runs a client command against this node: `--config` and `--web-port` are appended.
    pub fn cli(&self, args: &[&str]) -> Output {
        cli_in(args, &self.dir, self.port)
    }
}

impl Drop for CliNode {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Runs a client command with `--config <dir>/config.json --web-port <port>` appended.
pub fn cli_in(args: &[&str], dir: &Path, port: u16) -> Output {
    isolated(Command::new(env!("CARGO_BIN_EXE_meshtown")), dir)
        .args(args)
        .arg("--config")
        .arg(dir.join("config.json"))
        .args(["--web-port", &port.to_string()])
        .output()
        .expect("start meshtown")
}

/// Takes the environment of the person running the tests away from the process.
fn isolated(command: Command, dir: &Path) -> Command {
    let mut command = command;
    command
        .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE")
        .env_remove("MESHTOWN_CONFIG")
        .env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir)
        .env("APPDATA", dir);
    command
}

/// A fresh empty directory in the system temporary directory.
pub fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("mesh-cli-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary directory");
    dir
}

/// A port nobody is listening on right now.
pub fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    l.local_addr().unwrap().port()
}

/// The standard output of a finished command as text.
pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The standard error of a finished command as text.
pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn pump(stream: impl std::io::Read + Send + 'static, tx: Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

/// The port from the page address line.
fn page_port(line: &str) -> u16 {
    let url = line.split("http://").nth(1).expect("address in the line");
    let rest = url
        .strip_prefix("127.0.0.1:")
        .expect("address on 127.0.0.1");
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().expect("port")
}

/// Runs the program with exactly `args` and the isolated environment, in `dir`.
pub fn cli_bare(args: &[&str], dir: &Path) -> Output {
    isolated(Command::new(env!("CARGO_BIN_EXE_meshtown")), dir)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("start meshtown")
}
