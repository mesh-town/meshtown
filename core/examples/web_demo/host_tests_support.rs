//! Shared helpers for the demo host page tests: a host behind the real page and raw requests.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use meshtown_core::web_gui::{self, WebHandle, WebOptions};

use super::DemoHost;
use crate::args::Args;
use crate::config::prepare_demo_config;
use crate::shutdown::cleanup_dir;

pub(crate) const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";

/// The demo prepares one settings directory per process, so the page tests run one at a time.
pub(crate) static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A demo host behind the real page on a free loopback port.
pub(crate) struct Demo {
    pub(crate) host: Arc<DemoHost>,
    pub(crate) handle: WebHandle,
    pub(crate) dir: Option<PathBuf>,
    pub(crate) _turn: tokio::sync::MutexGuard<'static, ()>,
}

/// `--nets <nets>`; the page starts on port 0 with the fixed token.
pub(crate) async fn setup(nets: u8) -> Demo {
    setup_with(nets, &[]).await
}

/// `setup` with more flags, for example `--second-admin` so that this computer is not the last administrator.
pub(crate) async fn setup_with(nets: u8, more: &[&str]) -> Demo {
    let turn = ONE_AT_A_TIME.lock().await;
    let nets = nets.to_string();
    let mut flags = vec!["web_demo", "--nets", &nets, "--port", "0", "--token", TOKEN];
    flags.extend_from_slice(more);
    let args = Args::parse_from(flags);
    let (run, dir) = prepare_demo_config(&args, 1_700_000_000).expect("demo config");
    let main_node = Arc::new(crate::build_demo_node(&args, &run));
    let host = DemoHost::new(&args, &run, main_node);
    let mut opts = WebOptions::for_run(&run);
    opts.port = args.port;
    opts.fixed_token = args.token.clone();
    let handle = web_gui::start_web_page(opts, &run, host.clone())
        .await
        .unwrap_or_else(|e| panic!("the page did not start: {e}"));
    Demo {
        host,
        handle,
        dir,
        _turn: turn,
    }
}

impl Drop for Demo {
    fn drop(&mut self) {
        let _ = self.handle.stop.send(true);
        self.handle.task.abort();
        if let Some(dir) = &self.dir {
            cleanup_dir(dir);
        }
    }
}

impl Demo {
    pub(crate) fn slot_ids(&self) -> Vec<String> {
        use meshtown_core::web_gui::HostControl;
        let slots = self.host.slots();
        slots.iter().map(|s| s.id.as_str().to_string()).collect()
    }

    pub(crate) async fn get(&self, path: &str) -> (u16, Value) {
        self.send("GET", path, None).await
    }

    pub(crate) async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("POST", path, Some(body)).await
    }

    pub(crate) async fn put(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("PUT", path, Some(body)).await
    }

    /// `POST /api/nets`, returning the id of the slot the page created.
    pub(crate) async fn add_slot(&self) -> String {
        let (status, body) = self.post("/api/nets", json!({})).await;
        assert_eq!(status, 201, "{body}");
        body["slot"].as_str().expect("slot id").to_string()
    }

    pub(crate) async fn leave(&self, slot: &str) -> (u16, Value) {
        let path = format!("/api/net/{slot}/members/leave");
        self.post(&path, json!({"confirm": true})).await
    }

    /// One HTTP/1.1 request on a fresh connection; the reply body is read as JSON (`Null` if it is not).
    pub(crate) async fn send(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let port = self.handle.shared.port();
        let payload = body.map(|b| b.to_string()).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nx-meshtown-token: {TOKEN}\r\nOrigin: http://127.0.0.1:{port}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect to the page");
        stream.write_all(request.as_bytes()).await.expect("send");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.expect("read reply");
        parse_reply(&String::from_utf8_lossy(&raw))
    }
}

pub(crate) fn parse_reply(raw: &str) -> (u16, Value) {
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or_default();
    let json = match (body.find('{'), body.rfind('}')) {
        (Some(from), Some(to)) if from < to => serde_json::from_str(&body[from..=to]),
        _ => Ok(Value::Null),
    };
    (status, json.unwrap_or(Value::Null))
}

pub(crate) fn ids_of(nets: &Value) -> Vec<String> {
    let list = nets.as_array().expect("a nets array");
    list.iter()
        .map(|n| n["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

pub(crate) fn strings_of(list: &Value) -> Vec<String> {
    let items = list.as_array().expect("a list of strings");
    items
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// `main`, or six characters of `[a-z2-7]`.
pub(crate) fn is_slot_id(id: &str) -> bool {
    id == "main" || (id.len() == 6 && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7')))
}
