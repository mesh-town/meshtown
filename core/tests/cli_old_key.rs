//! A settings file whose network key is in an outdated form: the real binary still starts.
//!
//! What it does: runs `meshtown run --config <file>` with `mesh1-...` as the main network key, waits for the
//! page address, and checks that the process is alive, the journal says the key did not parse, the page answers
//! and `POST /api/network/create` makes a network afterwards.
//! What it doesn't do: it doesn't check the other outdated forms (see `cli_errors.rs` for the command line).

#[path = "common/node_web.rs"]
mod common;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

struct NodeProc {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
    dir: PathBuf,
}

impl NodeProc {
    fn start(config_json: &str) -> NodeProc {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!("mesh-old-key-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temporary directory");
        std::fs::write(dir.join("config.json"), config_json).expect("settings file");
        let mut child = Command::new(env!("CARGO_BIN_EXE_meshtown"))
            .args(["run", "--config"])
            .arg(dir.join("config.json"))
            .args(["--web-port", "0", "--no-checks", "--no-socks"])
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
        NodeProc {
            child,
            lines: rx,
            seen: Vec::new(),
            dir,
        }
    }

    /// Waits for a line with `needle`; the lines read are accumulated in `seen`.
    fn wait_line(&mut self, needle: &str, limit: Duration) -> Option<String> {
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
}

impl Drop for NodeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn pump(stream: impl std::io::Read + Send + 'static, tx: std::sync::mpsc::Sender<String>) {
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

#[test]
fn an_old_key_in_the_file_starts_the_node_without_a_network() {
    // given: a settings file with a key in the outdated three-word form
    let key = format!("mesh1-{}", "a".repeat(52));
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{key}"}}}}}}"#
    );
    // when: the node starts
    let mut node = NodeProc::start(&config);
    let line = node
        .wait_line("?token=", Duration::from_secs(30))
        .expect("the page address was not printed");
    let port = page_port(&line);
    let log = node
        .wait_line("network key did not parse: ", Duration::from_secs(10))
        .expect("the journal line about the key");
    // then: the journal says it starts without a network, the process is alive
    assert!(log.contains("; starting without a network"), "{log}");
    assert!(node.child.try_wait().unwrap().is_none(), "exited early");
    let token = std::fs::read_to_string(node.dir.join("web-token"))
        .expect("web-token")
        .trim()
        .to_string();
    // and the page answers, with no network
    let (code, state) = common::api(port, &token, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(state["network"].is_null(), "{state}");
    // and a network can be created afterwards
    let body = Some(serde_json::json!({"label": "fresh"}));
    let (code, reply) = common::api(port, &token, "POST", "/api/network/create", body);
    assert_eq!(code, 200, "{reply}");
    let (code, state) = common::api(port, &token, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(!state["network"].is_null(), "{state}");
}
