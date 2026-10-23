//! A node stand-in for the tray tests: a TCP server on `127.0.0.1` that answers fixed bodies by method and
//! path and records every request, plus builders of the JSON the node sends (`/api/state`, `/api/overview`).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// One request the stub has received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) body: String,
}

type Answer = (u16, String);

#[derive(Default)]
struct Route {
    once: VecDeque<Answer>,
    always: Option<Answer>,
}

#[derive(Default)]
struct Inner {
    routes: HashMap<(String, String), Route>,
    hits: Vec<Hit>,
}

/// The fake node. A path nobody set up answers `404 not_found`, like a node that does not know the path.
pub(crate) struct StubNode {
    port: u16,
    inner: Arc<Mutex<Inner>>,
    task: tokio::task::JoinHandle<()>,
}

impl StubNode {
    pub(crate) async fn start() -> StubNode {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the stub");
        let port = listener.local_addr().unwrap().port();
        let inner = Arc::new(Mutex::new(Inner::default()));
        let shared = inner.clone();
        let task = crate::node::output::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                crate::node::output::spawn(serve(stream, shared.clone()));
            }
        });
        StubNode { port, inner, task }
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Every later request for `method path` gets this answer.
    pub(crate) fn reply(&self, method: &str, path: &str, status: u16, body: &Value) {
        let mut inner = self.inner.lock().unwrap();
        let route = inner.routes.entry(key(method, path)).or_default();
        route.always = Some((status, body.to_string()));
    }

    /// The next request for `method path` gets this answer, then the usual one.
    pub(crate) fn reply_once(&self, method: &str, path: &str, status: u16, body: &Value) {
        let mut inner = self.inner.lock().unwrap();
        let route = inner.routes.entry(key(method, path)).or_default();
        route.once.push_back((status, body.to_string()));
    }

    pub(crate) fn hits(&self, method: &str, path: &str) -> Vec<Hit> {
        let inner = self.inner.lock().unwrap();
        let same = |h: &&Hit| h.method == method && h.path == path;
        inner.hits.iter().filter(same).cloned().collect()
    }

    pub(crate) fn count(&self, method: &str, path: &str) -> usize {
        self.hits(method, path).len()
    }

    /// Waits until at least `n` requests for `method path` have arrived.
    pub(crate) async fn wait_count(&self, method: &str, path: &str, n: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while self.count(method, path) < n {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for {n} x {method} {path}; got {:?}",
                self.inner.lock().unwrap().hits
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

impl Drop for StubNode {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn key(method: &str, path: &str) -> (String, String) {
    (method.to_string(), path.to_string())
}

async fn serve(mut stream: TcpStream, inner: Arc<Mutex<Inner>>) {
    let Some(hit) = read_request(&mut stream).await else {
        return;
    };
    let answer = answer_for(&inner, hit);
    let (status, body) = answer;
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn answer_for(inner: &Mutex<Inner>, hit: Hit) -> Answer {
    let mut inner = inner.lock().unwrap();
    let wanted = key(&hit.method, &hit.path);
    inner.hits.push(hit);
    let Some(route) = inner.routes.get_mut(&wanted) else {
        return not_found();
    };
    route
        .once
        .pop_front()
        .or_else(|| route.always.clone())
        .unwrap_or_else(not_found)
}

fn not_found() -> Answer {
    let body = json!({"error": "not_found", "message": "no such address"});
    (404, body.to_string())
}

async fn read_request(stream: &mut TcpStream) -> Option<Hit> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(hit) = complete_request(&buf) {
            return Some(hit);
        }
    }
}

/// The request if all of its bytes (headers and the `Content-Length` body) are in `buf`.
fn complete_request(buf: &[u8]) -> Option<Hit> {
    let text = String::from_utf8_lossy(buf);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let wanted = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().ok())
        })
        .flatten()
        .unwrap_or(0);
    if body.len() < wanted {
        return None;
    }
    let mut line = head.lines().next()?.split_whitespace();
    let (method, path) = (line.next()?, line.next()?);
    Some(Hit {
        method: method.to_string(),
        path: path.to_string(),
        body: body.to_string(),
    })
}

#[path = "stub_samples.rs"]
mod samples;
pub(crate) use samples::*;
