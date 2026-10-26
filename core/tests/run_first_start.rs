//! First start of `meshtown run` without a network key: the real binary, not a bypass via `commands::run_node_command`.
//! The node does not create a network — it brings up the page, waits for sign-in and leaves the settings file without a key.
//! With the page turned off in the file, it exits with an error and a hint about `--invite`.

#[path = "common/node_web.rs"]
mod common;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Child `meshtown run`: lines of both streams go into one channel tagged with the stream.
struct NodeProc {
    child: Child,
    lines: Receiver<(bool, String)>,
    seen: std::cell::RefCell<Vec<(bool, String)>>,
    dir: PathBuf,
}

impl NodeProc {
    fn start(config_json: &str, tag: &str) -> NodeProc {
        NodeProc::start_on(config_json, tag, 0)
    }

    fn make_dir(config_json: &str, tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mesh-first-start-{tag}-{}-{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).expect("temporary directory");
        std::fs::write(dir.join("config.json"), config_json).expect("settings file");
        dir
    }

    fn start_on(config_json: &str, tag: &str, web_port: u16) -> NodeProc {
        let dir = NodeProc::make_dir(config_json, tag);
        let mut child = Command::new(env!("CARGO_BIN_EXE_meshtown"))
            .args(["run", "--config"])
            .arg(dir.join("config.json"))
            .args([
                "--web-port",
                &web_port.to_string(),
                "--no-checks",
                "--no-socks",
            ])
            .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
            .env_remove("MESHTOWN_KEY")
            .env_remove("MESHTOWN_INVITE")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start meshtown");
        let (tx, rx) = channel();
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        for (is_err, stream) in [
            (false, Box::new(out) as Box<dyn std::io::Read + Send>),
            (true, Box::new(err) as Box<dyn std::io::Read + Send>),
        ] {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if tx.send((is_err, line)).is_err() {
                        break;
                    }
                }
            });
        }
        NodeProc {
            child,
            lines: rx,
            seen: Default::default(),
            dir,
        }
    }

    /// Waits for a line with `needle`; the lines read are accumulated in `seen`, not lost.
    fn wait_line(&self, needle: &str, timeout: Duration) -> Option<String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok((is_err, line)) => {
                    self.seen.borrow_mut().push((is_err, line.clone()));
                    if line.contains(needle) {
                        return Some(line);
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// Everything already read from the stream (`stderr` — true), as one text.
    fn text(&self, stderr: bool) -> String {
        let seen = self.seen.borrow();
        seen.iter()
            .filter(|(e, _)| *e == stderr)
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Waits for the process to exit; `None` — it did not exit within `timeout`.
    fn wait_exit(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(st)) = self.child.try_wait() {
                std::thread::sleep(Duration::from_millis(200));
                while let Ok((e, l)) = self.lines.try_recv() {
                    self.seen.borrow_mut().push((e, l));
                }
                return Some(st.code().unwrap_or(-1));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn config_text(&self) -> String {
        std::fs::read_to_string(self.dir.join("config.json")).unwrap_or_default()
    }
}

impl Drop for NodeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn has_no_key(config: &str) -> bool {
    let v: serde_json::Value = serde_json::from_str(config).expect("settings file is JSON");
    v.pointer("/config/network/key")
        .is_none_or(|k| k.is_null() || k == "")
}

/// The address with a temporary token from the node output: port and token.
fn temp_entry(node: &NodeProc) -> (u16, String) {
    let line = node
        .wait_line("?token=", Duration::from_secs(30))
        .expect("address with a temporary token was not printed");
    let url = line.split("http://").nth(1).expect("address in the line");
    let url = url.split_whitespace().next().unwrap();
    let rest = url
        .strip_prefix("127.0.0.1:")
        .expect("address on 127.0.0.1");
    let (port, token) = rest
        .split_once("/?token=")
        .expect("port and token in the address");
    (port.parse().expect("port"), token.to_string())
}

/// The permanent token from the `web-token` file next to the settings file.
fn file_token(node: &NodeProc) -> String {
    let text = std::fs::read_to_string(node.dir.join("web-token")).expect("web-token");
    text.trim().to_string()
}

#[test]
fn run_without_key_waits_on_page_and_keeps_file_keyless() {
    let node = NodeProc::start(
        r#"{"config":{"version":1,"web":{"access":"localhost"}}}"#,
        "keyless",
    );
    let (port, temp) = temp_entry(&node);
    node.wait_line("You haven't joined a", Duration::from_secs(10))
        .expect("the \"You haven't joined a\" line did not appear");
    let hint = node
        .wait_line("--invite", Duration::from_secs(10))
        .expect("hint about --invite");
    assert!(
        hint.contains("[node]"),
        "second line is from [node]: {hint}"
    );
    let perm = file_token(&node);
    assert_ne!(
        perm, temp,
        "the address must have the temporary token, not the permanent one"
    );
    let (code, _) = common::api(port, &temp, "GET", "/api/state", None);
    assert_eq!(code, 401, "a temporary token does not work as a header");
    let (code, state) = common::api(port, &perm, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(state["network"].is_null(), "no network: {state}");
    assert_eq!(state["node"]["status"], "offline", "{state}");
    assert!(
        has_no_key(&node.config_text()),
        "there must be no key in the file"
    );
    assert_eq!(
        node.text(false).matches("?token=").count(),
        1,
        "there must be exactly one address with a token"
    );
    for text in [node.text(false), node.text(true)] {
        assert!(!text.contains(&perm), "permanent token was printed");
        assert!(
            !text.contains("new network created"),
            "no auto-creation: {text}"
        );
    }
}

#[test]
fn run_without_key_temp_token_exchanges_once() {
    let node = NodeProc::start(
        r#"{"config":{"version":1,"web":{"access":"localhost"}}}"#,
        "exchange",
    );
    let (port, temp) = temp_entry(&node);
    let body = Some(serde_json::json!({"token": temp}));
    let (first, _) = common::api(port, "", "POST", "/api/auth/token", body.clone());
    assert_eq!(first, 200, "exchange of the temporary token");
    let (second, _) = common::api(port, "", "POST", "/api/auth/token", body);
    assert_eq!(second, 401, "a temporary token is single-use");
}

#[test]
fn run_without_key_and_web_disabled_fails_with_invite_hint() {
    let mut node = NodeProc::start(
        r#"{"config":{"version":1,"web":{"enabled":false}}}"#,
        "noweb",
    );
    let code = node
        .wait_exit(Duration::from_secs(20))
        .expect("the node must exit on its own");
    assert_ne!(code, 0);
    let err = node.text(true);
    assert!(
        err.contains("the page is turned off in Mesh Town settings"),
        "stderr: {err}"
    );
    assert!(!err.contains("--web-port"), "stderr: {err}");
    assert!(err.contains("web.enabled"), "stderr: {err}");
    assert!(err.contains("--invite"), "stderr: {err}");
    assert!(
        has_no_key(&node.config_text()),
        "there must be no key in the file"
    );
}

#[test]
fn run_without_key_busy_web_port_advises_port() {
    let holder = std::net::TcpListener::bind("127.0.0.1:0").expect("occupy the port");
    let port = holder.local_addr().unwrap().port();
    // The node comes up on 127.0.0.1 (`access: localhost`): the busy address is the same as `holder`.
    let config = r#"{"config":{"version":1,"web":{"access":"localhost"}}}"#;
    let mut node = NodeProc::start_on(config, "busyweb", port);
    let code = node
        .wait_exit(Duration::from_secs(20))
        .expect("the node must exit on its own");
    assert_ne!(code, 0);
    let err = node.text(true);
    assert!(err.contains("--web-port"), "stderr: {err}");
    assert!(err.contains("--invite"), "stderr: {err}");
    assert!(
        !err.contains("Mesh Town is running without the page"),
        "stderr: {err}"
    );
}
