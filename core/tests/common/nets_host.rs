//! Scaffolding for scenarios that need the real process host (several network slots behind one page):
//! a node with a key started through `Host::run_with_saved_slots` on the in-process test network,
//! its page on a free loopback port, and the HTTP helper of `common/node_web.rs`.
//! Included as `#[path = "common/nets_host.rs"] mod nets_host;` next to `mod common;` (node_web).
#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule};
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::host::{Host, PageRole};
use meshtown_core::node::node_ctl::{NodeCtl, Relaunch};
use meshtown_core::node::{self, output, IdentityState};
use meshtown_core::web_gui::TOKEN_FILE_NAME;
use serde_json::{json, Value};

use crate::common::{api, key_text, port_open, wait_until};

/// A node run by the process host, with its page.
pub(crate) struct HostNode {
    pub(crate) dir: PathBuf,
    pub(crate) port: u16,
    pub(crate) token: String,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

fn free_port() -> u16 {
    let socket = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a free port");
    socket.local_addr().expect("local address").port()
}

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("mesh-nets-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Starts a node in the network `key` under the process host; returns when the page answers.
pub(crate) async fn boot_host(tag: &str, key: &NetworkKey) -> HostNode {
    let dir = scratch_dir(tag);
    let port = free_port();
    let overrides = CliOverrides {
        network_key: Some(key_text(key)),
        checks_enabled: Some(false),
        socks_port: Some(0),
        web_port: Some(port),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut run = config_file::prepare(&source, &overrides, true, 0).expect("boot_host: prepare");
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    let relaunch = Relaunch {
        test_net: Some(meshtown_core::transport::net::mem::process_net()),
        file: run.file.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
    };
    let mut cfg = relaunch.node_config(
        &run.effective_settings,
        meshtown_core::members::network_key::parse_user_input(&key_text(key))
            .expect("a managed key"),
    );
    cfg.geoip = false;
    cfg.identity = meshtown_core::node::IdentityState::from_report(&run.file_report);
    let ctl = NodeCtl::new(&run.effective_settings);
    let saved = Some(relaunch.clone());
    let body = Host::run_with_saved_slots(run, ctl, PageRole::Optional, saved, |run, ctl| {
        meshtown_core::node::run_node_with_ctl(cfg, run.effective_settings, ctl, relaunch)
    });
    let task = output::spawn(body);
    let node = HostNode {
        token: wait_token(&dir).await,
        dir,
        port,
        task,
    };
    wait_page(&node).await;
    node
}

/// Starts a node without a network under the process host; it waits for the page to give it one.
pub(crate) async fn boot_host_empty(tag: &str) -> HostNode {
    let dir = scratch_dir(tag);
    let port = free_port();
    let overrides = CliOverrides {
        checks_enabled: Some(false),
        socks_port: Some(0),
        web_port: Some(port),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut run = config_file::prepare(&source, &overrides, true, 0).expect("boot_host_empty");
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    let relaunch = Relaunch {
        test_net: Some(meshtown_core::transport::net::mem::process_net()),
        file: run.file.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
    };
    let identity = IdentityState::from_report(&run.file_report);
    let ctl = NodeCtl::new_without_network(&run.effective_settings);
    let saved = Some(relaunch.clone());
    let body = Host::run_with_saved_slots(run, ctl, PageRole::Optional, saved, move |run, ctl| {
        node::run_node_awaiting_key(run.effective_settings, identity, ctl, relaunch.clone())
    });
    let task = output::spawn(body);
    let node = HostNode {
        token: wait_token(&dir).await,
        dir,
        port,
        task,
    };
    wait_page(&node).await;
    node
}

async fn wait_token(dir: &std::path::Path) -> String {
    let path = dir.join(TOKEN_FILE_NAME);
    let read = || {
        std::fs::read_to_string(&path)
            .ok()
            .filter(|t| !t.trim().is_empty())
    };
    assert!(
        wait_until(Duration::from_secs(10), || read().is_some()).await,
        "the page token file did not appear"
    );
    read().unwrap().trim().to_string()
}

async fn wait_page(node: &HostNode) {
    let ok = wait_until(Duration::from_secs(10), || {
        if !port_open(node.port) {
            return false;
        }
        let (status, v) = node.get("/api/state");
        status == 200 && v["node"]["status"] != json!("starting")
    })
    .await;
    assert!(
        ok,
        "the page did not come up with a running node within 10 s"
    );
}

impl HostNode {
    pub(crate) fn get(&self, path: &str) -> (u16, Value) {
        api(self.port, &self.token, "GET", path, None)
    }

    pub(crate) fn post(&self, path: &str, body: Value) -> (u16, Value) {
        api(self.port, &self.token, "POST", path, Some(body))
    }

    /// Stops the process through the page and waits for the host to return.
    pub(crate) async fn teardown(self) {
        let (status, _) = self.post("/api/actions/stop", json!({}));
        assert_eq!(status, 202, "teardown: stop must return 202");
        let res = tokio::time::timeout(Duration::from_secs(15), self.task).await;
        let _ = std::fs::remove_dir_all(&self.dir);
        assert!(
            matches!(res, Ok(Ok(Ok(())))),
            "teardown: the host did not finish in time or finished with an error"
        );
    }
}
