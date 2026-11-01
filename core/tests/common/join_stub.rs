//! A fake node page for the `join` command tests. Included via `#[path]`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A page that answers fixed replies and records every request line (`"POST /api/nets"`), for the cases a real
/// node does not produce on demand. The token file is written to `dir` so the client finds it.
pub struct StubPage {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<String>>>,
}

/// One canned answer: method and path, status, JSON body.
pub type Canned = (&'static str, &'static str, u16, &'static str);

impl StubPage {
    pub fn start(dir: &Path, answers: &'static [Canned]) -> StubPage {
        std::fs::write(dir.join("web-token"), "stub-token").expect("token file");
        let listener = TcpListener::bind("127.0.0.1:0").expect("port");
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                answer(stream, answers, &log);
            }
        });
        StubPage { port, requests }
    }

    pub fn seen(&self, line: &str) -> bool {
        self.requests.lock().unwrap().iter().any(|r| r == line)
    }
}

fn answer(mut stream: TcpStream, answers: &[Canned], log: &Mutex<Vec<String>>) {
    let line = request_line(&mut stream);
    log.lock().unwrap().push(line.clone());
    let found = answers.iter().find(|a| format!("{} {}", a.0, a.1) == line);
    let (status, body) = found.map_or((404, "{}"), |a| (a.2, a.3));
    let reply = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes());
}

/// `"METHOD path"` of the request; the rest of the head and the body are read and dropped.
fn request_line(stream: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    while !raw.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
        raw.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&raw).into_owned();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::to_string)
        })
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = stream.read_exact(&mut body);
    let first = head.lines().next().unwrap_or("");
    first.split(' ').take(2).collect::<Vec<_>>().join(" ")
}
