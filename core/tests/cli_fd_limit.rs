//! The `open files limit` log line belongs to `run`/`tray` only: one-line commands keep stdout clean.
//! The one positive case: a real `meshtown run` must print it.
//! The child starts with a low soft `RLIMIT_NOFILE`, so the limit is always raised at its startup.
#![cfg(unix)]

use std::os::unix::process::CommandExt;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Starts the child with a low soft `RLIMIT_NOFILE` (128), so the limit is always raised at startup.
fn lower_nofile(cmd: &mut Command) {
    // SAFETY: only async-signal-safe libc calls between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
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
    cmd.args(args).env_remove("MESHTOWN_KEY").env_remove("MESHTOWN_INVITE");
    lower_nofile(&mut cmd);
    cmd.output().expect("start meshtown")
}

fn stdout_of(args: &[&str]) -> String {
    String::from_utf8_lossy(&mesh_low_limit(args).stdout).into_owned()
}

#[test]
fn keygen_raw_stdout_is_one_line() {
    let out = stdout_of(&["keygen", "--raw"]);
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

#[test]
fn run_reports_open_files_limit() {
    let key = meshtown::network_key::NetworkKey::generate().unwrap().encode();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_meshtown"));
    cmd.args(["run", "--no-config", "--key", &key, "--web-port", "0"])
        .args(["--no-relay", "--no-checks", "--no-socks"])
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
