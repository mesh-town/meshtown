//! A settings file written by an older version still holds `transport.mode` (and `meshtown run` once took
//! `--relay-only` / `--no-relay`). The real binary must start with such a file, say that the field is
//! deprecated, and ignore it.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use meshtown_core::members::network_key::NetworkKey;

/// The time from launch to the end of the start phase.
const START_LIMIT: Duration = Duration::from_secs(30);
/// The line the node prints at the end of its start phase.
const READY_LINE: &str = "serving as exit for members";
/// The warning about the retired field.
const RETIRED_LINE: &str =
    "deprecated fields ignored and dropped on the next write: transport.mode";

struct Proc {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
    dir: PathBuf,
}

impl Proc {
    fn start(config_json: &str) -> Proc {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mesh-retired-config-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temporary directory");
        std::fs::write(dir.join("config.json"), config_json).expect("settings file");
        let key = NetworkKey::generate().unwrap().encode();
        let mut child = Command::new(env!("CARGO_BIN_EXE_meshtown"))
            .args(["run", "--config"])
            .arg(dir.join("config.json"))
            .args([
                "--key",
                &key,
                "--web-port",
                "0",
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
        pump(child.stdout.take().unwrap(), tx.clone());
        pump(child.stderr.take().unwrap(), tx);
        Proc {
            child,
            lines: rx,
            seen: Vec::new(),
            dir,
        }
    }

    /// Waits for a line with `needle`; what has been read accumulates in `seen`.
    fn wait_line(&mut self, needle: &str) -> bool {
        let deadline = Instant::now() + START_LIMIT;
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
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
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
fn old_config_with_transport_mode_starts_and_warns() {
    // given: a settings file with the retired `transport.mode`
    let json = r#"{"rev": 1, "config": {"version": 1, "transport": {"mode": "relay_only"}}}"#;
    // when: the real binary starts with it
    let mut node = Proc::start(json);
    // then: the start phase ends, and the log has the warning
    assert!(
        node.wait_line(READY_LINE),
        "the node did not finish the start; got {:?}",
        node.seen
    );
    assert!(
        node.seen.iter().any(|l| l.contains(RETIRED_LINE)),
        "no warning in {:?}",
        node.seen
    );
}
