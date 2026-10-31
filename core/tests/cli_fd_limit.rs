//! The `open files limit` log line belongs to `run`/`tray` only: one-line commands keep stdout clean.
//! The one positive case: a real `meshtown run` must print it.
//! The child starts with a low soft `RLIMIT_NOFILE`, so the limit is always raised at its startup.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Starts the child with a low soft `RLIMIT_NOFILE` (128), so the limit is always raised at startup.
fn lower_nofile(cmd: &mut Command) {
    // SAFETY: only async-signal-safe libc calls between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            let mut lim = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            lim.rlim_cur = 128.min(lim.rlim_max);
            if libc::setrlimit(libc::RLIMIT_NOFILE, &lim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn mesh_low_limit(args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_meshtown"));
    cmd.args(args)
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE");
    lower_nofile(&mut cmd);
    cmd.output().expect("start meshtown")
}

fn stdout_of(args: &[&str]) -> String {
    String::from_utf8_lossy(&mesh_low_limit(args).stdout).into_owned()
}

#[test]
fn key_only_raw_stdout_is_one_line() {
    let dir = std::env::temp_dir().join(format!("cli_fd_limit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let config = dir.join("config.json");
    let args = [
        "network",
        "create",
        "--key-only",
        "--raw",
        "--config",
        config.to_str().unwrap(),
    ];
    let out = stdout_of(&args);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.contains("open files limit"), "stdout: {out}");
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "stdout: {out}");
}

#[test]
fn config_path_has_no_limit_line() {
    let out = stdout_of(&["config-path"]);
    assert!(!out.contains("open files limit"), "stdout: {out}");
}

#[test]
fn help_has_no_limit_line() {
    let out = stdout_of(&["--help"]);
    assert!(!out.contains("open files limit"), "stdout: {out}");
}

/// Kills and reaps the child on any outcome (success, timeout, panic).
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawns `meshtown run` (low soft limit, no relay/checks/socks) and returns the reaper plus a line channel.
fn spawn_run(extra: &[&str]) -> (Reaper, std::sync::mpsc::Receiver<String>) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_meshtown"));
    cmd.arg("run")
        .args(extra)
        .args([
            "--web-port",
            "0",
            "--set",
            "checks.enabled=false",
            "--no-socks",
        ])
        .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    lower_nofile(&mut cmd);
    let mut node = Reaper(cmd.spawn().expect("start meshtown"));
    let stdout = node.0.stdout.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    (node, rx)
}

fn next_line(rx: &std::sync::mpsc::Receiver<String>) -> String {
    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => return line,
            Err(e) => panic!("no stdout line from `run`: {e}"),
        }
    }
}

fn assert_version_line(line: &str) {
    let want = format!("[node] meshtown {}", env!("CARGO_PKG_VERSION"));
    assert!(line.ends_with(&want), "first line: {line}");
}

#[test]
fn run_prints_version_before_the_limit_line() {
    let key = managed_key_text();
    let (_node, rx) = spawn_run(&["--no-config", "--key", &key]);
    assert_version_line(&next_line(&rx));
    let second = next_line(&rx);
    assert!(second.contains("open files limit"), "second line: {second}");
}

#[test]
fn run_without_key_still_starts_with_the_version() {
    let (_node, rx) = spawn_run(&["--no-config"]);
    assert_version_line(&next_line(&rx));
}

#[test]
fn version_flag_stdout_is_one_line() {
    let out = stdout_of(&["--version"]);
    assert!(!out.contains("open files limit"), "stdout: {out}");
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "stdout: {out}");
    assert_eq!(
        lines[0].trim(),
        format!("meshtown {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn run_reports_open_files_limit() {
    let key = managed_key_text();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_meshtown"));
    cmd.args(["run", "--no-config", "--key", &key, "--web-port", "0"])
        .args(["--set", "checks.enabled=false", "--no-socks"])
        .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    lower_nofile(&mut cmd);
    let mut node = Reaper(cmd.spawn().expect("start meshtown"));
    let stdout = node.0.stdout.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) if line.contains("open files limit") => return,
            Ok(line) => seen.push(line),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                panic!("no `open files limit` line in stdout: {seen:#?}")
            }
        }
    }
}

/// A Managed network key of a fresh base, created by an arbitrary node.
fn managed_key_text() -> String {
    use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
    use meshtown_core::transport::net::NodeSecret;
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    managed_key_with_label(None, &base, &root)
}
