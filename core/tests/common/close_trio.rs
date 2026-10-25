//! The helpers of the close loopback tests: three nodes of one Managed network under the process host, each with
//! its page, started like the production relaunch does but on the in-process test network.
//!
//! What it does: boots the creator, lets two nodes in by an invite from the page, makes one an administrator.
//! What it doesn't do: no assertions about the close itself; the tests own them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use meshtown_core::config::config_file::{
    self, save_config_file, CliOverrides, ConfigSource, ListenAccess, PathRule, TransportMode,
};
use meshtown_core::members::network_key::{
    managed_key, managed_key_with_label, NetKind, NetworkKey,
};
use meshtown_core::node::host::{Host, PageRole};
use meshtown_core::node::node_ctl::{NodeCtl, Relaunch};
use meshtown_core::node::{self, output, IdentityState};
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::PeerId;
use meshtown_core::transport::net_mainline::unix_now;
use meshtown_core::web_gui::TOKEN_FILE_NAME;
use serde_json::{json, Value};

use super::common::{api, wait_until};

/// How long a view or a record may take to spread.
pub const VIEW_LIMIT: Duration = Duration::from_secs(25);
/// How long the nodes may take to link to each other after they joined.
const LINK_LIMIT: Duration = Duration::from_secs(60);
pub const LABEL: &str = "club";

/// The creator, an administrator and a plain member of one Managed network.
pub struct Trio {
    pub root: Hosted,
    pub admin: Hosted,
    pub member: Hosted,
}

/// Starts three nodes; the others join by the creator's invite, then the creator makes one of them an administrator.
pub async fn trio() -> Trio {
    let base = NetworkKey::generate().unwrap().to_bytes();
    let root = boot_root(&base).await;
    let admin = joined_by_invite(&root, "close-admin").await;
    let member = joined_by_invite(&root, "close-member").await;
    let (status, body) = root.post(
        "/api/members/admin",
        json!({"id": admin.id_full(), "admin": true}),
    );
    assert_eq!(status, 200, "make the administrator: {body}");
    let t = Trio {
        root,
        admin,
        member,
    };
    let linked = wait_until(LINK_LIMIT, || t.all().all(|n| n.fully_linked())).await;
    assert!(
        linked,
        "the nodes did not all link to each other: {:?}",
        t.all().map(|n| n.get("/api/members").1).collect::<Vec<_>>()
    );
    let ready = wait_until(VIEW_LIMIT, || {
        t.admin.role().as_deref() == Some("admin") && t.member.role().as_deref() == Some("member")
    })
    .await;
    assert!(
        ready,
        "the roles did not reach the nodes: {:?}",
        t.all_status()
    );
    t
}

impl Trio {
    pub fn all(&self) -> impl Iterator<Item = &Hosted> {
        [&self.root, &self.admin, &self.member].into_iter()
    }

    pub fn all_notices(&self) -> Vec<Vec<Value>> {
        self.all().map(Hosted::notices).collect()
    }

    pub fn all_status(&self) -> Vec<Value> {
        self.all()
            .map(|n| n.get("/api/state").1["node"]["status"].clone())
            .collect()
    }

    pub async fn teardown(self) {
        self.member.teardown().await;
        self.admin.teardown().await;
        self.root.teardown().await;
    }
}

/// A node run by the process host, with its page.
pub struct Hosted {
    dir: PathBuf,
    port: u16,
    token: String,
    peer: PeerId,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl Hosted {
    pub fn get(&self, path: &str) -> (u16, Value) {
        api(self.port, &self.token, "GET", path, None)
    }

    pub fn post(&self, path: &str, body: Value) -> (u16, Value) {
        api(self.port, &self.token, "POST", path, Some(body))
    }

    pub fn id_full(&self) -> String {
        self.peer.to_string()
    }

    pub fn notices(&self) -> Vec<Value> {
        let state = self.get("/api/state").1;
        state["notices"].as_array().cloned().unwrap_or_default()
    }

    pub fn role(&self) -> Option<String> {
        // The members page recounts the list; the state page only shows the last published count.
        let _ = self.get("/api/members");
        let state = self.get("/api/state").1;
        state["network"]["me_role"].as_str().map(str::to_string)
    }

    pub fn is_offline(&self) -> bool {
        // The status alone flickers with the links; a node without a network has no `network` block.
        let state = self.get("/api/state").1;
        state["network"].is_null() && state["node"]["status"] == json!("offline")
    }

    /// Whether the members page shows both other nodes online.
    pub fn fully_linked(&self) -> bool {
        let members = self.get("/api/members").1;
        let rows = members["members"].as_array().cloned().unwrap_or_default();
        let others = rows.iter().filter(|r| r["self"] == json!(false));
        others.clone().count() == 2 && others.into_iter().all(|r| r["online"] == json!(true))
    }

    /// Whether the members page lists `who` with a status.
    pub fn lists(&self, who: &Hosted) -> bool {
        let members = self.get("/api/members").1;
        members["members"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|r| r["id_full"] == json!(who.id_full())))
    }

    /// Stops the process through the page and waits for the host to return.
    pub async fn teardown(self) {
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

/// The prepared run of a fresh node: its directory, page port and the relaunch data.
pub struct Prepared {
    dir: PathBuf,
    port: u16,
    run: config_file::RunConfig,
    relaunch: Relaunch,
}

/// A run in a fresh directory; the key made by `key` from the node's own id is written into the settings file
/// first (never given as a flag, so the node can leave the network).
fn prepare(tag: &str, key: Option<&dyn Fn(PeerId) -> String>) -> Prepared {
    let dir = scratch_dir(tag);
    let port = free_port();
    let overrides = CliOverrides {
        transport_mode: Some(TransportMode::NoRelay),
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
    let fresh = config_file::prepare(&source, &overrides, true, 0).expect("prepare a fresh file");
    if let Some(key) = key {
        write_key(&dir, &fresh, &key(fresh.secret.peer_id()));
    }
    let mut run = config_file::prepare(&source, &overrides, true, 0).expect("prepare the run");
    run.effective_settings.process.web_access = ListenAccess::Localhost;
    let relaunch = Relaunch {
        test_net: Some(process_net()),
        file: run.file.clone(),
        cli: overrides,
        secret: run.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
    };
    Prepared {
        dir,
        port,
        run,
        relaunch,
    }
}

/// Runs the host of `p` with `body` as the node cycle and waits for its page.
async fn serve<F, Fut>(p: Prepared, ctl: std::sync::Arc<NodeCtl>, body: F) -> Hosted
where
    F: Fn(config_file::RunConfig, std::sync::Arc<NodeCtl>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let peer = p.run.secret.peer_id();
    let saved = Some(p.relaunch.clone());
    let host = Host::run_with_saved_slots(p.run, ctl, PageRole::Optional, saved, body);
    let task = output::spawn(host);
    let node = Hosted {
        token: wait_token(&p.dir).await,
        dir: p.dir,
        port: p.port,
        peer,
        task,
    };
    wait_page(&node).await;
    node
}

/// Starts the creator of the Managed network made of `base`.
async fn boot_root(base: &[u8; 32]) -> Hosted {
    let key = |me: PeerId| managed_key_with_label(Some(LABEL), base, &me);
    let p = prepare("close-root", Some(&key));
    let creator = p.run.secret.peer_id();
    let relaunch = p.relaunch.clone();
    let mut cfg = relaunch.node_config(&p.run.effective_settings, managed_key(base, &creator));
    cfg.geoip = false;
    cfg.open_list = false;
    cfg.net_kind = NetKind::Managed { root: creator };
    cfg.reply_secret = NetworkKey::from_bytes(*base);
    cfg.identity = IdentityState::from_report(&p.run.file_report);
    let ctl = NodeCtl::new(&p.run.effective_settings);
    serve(p, ctl, move |run, ctl| {
        node::run_node_with_ctl(cfg.clone(), run.effective_settings, ctl, relaunch.clone())
    })
    .await
}

/// Starts a node without a network; it waits for the page to give it one.
async fn boot_joiner(tag: &str) -> Hosted {
    let p = prepare(tag, None);
    let relaunch = p.relaunch.clone();
    let identity = IdentityState::from_report(&p.run.file_report);
    let ctl = NodeCtl::new_without_network(&p.run.effective_settings);
    serve(p, ctl, move |run, ctl| {
        node::run_node_awaiting_key(run.effective_settings, identity, ctl, relaunch.clone())
    })
    .await
}

/// A node without a network that joins through an invite of `root` and waits until it is a member.
async fn joined_by_invite(root: &Hosted, tag: &str) -> Hosted {
    let node = boot_joiner(tag).await;
    let ready = wait_until(VIEW_LIMIT, || root.role().as_deref() == Some("admin")).await;
    assert!(ready, "the creator did not see itself as an administrator");
    let (status, body) = root.get("/api/network/invite");
    assert_eq!(status, 200, "the invite: {body}");
    let invite = body["invite"].as_str().expect("an invite string");
    let (status, body) = node.post("/api/network/join", json!({ "key": invite }));
    assert!(status == 200 || status == 202, "join: {status} {body}");
    let member = wait_until(VIEW_LIMIT, || root.lists(&node) && node.role().is_some()).await;
    assert!(
        member,
        "the node did not join: {}",
        node.get("/api/state").1
    );
    node
}

/// Writes `key` as the network key of the settings file the run was prepared from.
fn write_key(dir: &Path, run: &config_file::RunConfig, key: &str) {
    let mut settings = run.file_settings.clone();
    settings.net.network_key = Some(key.to_string());
    let path = dir.join(config_file::FILE_NAME);
    let saved = save_config_file(
        &path,
        Some(run.file_report.rev),
        unix_now(),
        &run.secret,
        &settings,
    );
    saved.expect("write the network key into the settings file");
}

fn free_port() -> u16 {
    let socket = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a free port");
    socket.local_addr().expect("local address").port()
}

fn scratch_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-close-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

async fn wait_token(dir: &Path) -> String {
    let path = dir.join(TOKEN_FILE_NAME);
    let read = || {
        std::fs::read_to_string(&path)
            .ok()
            .filter(|t| !t.trim().is_empty())
    };
    let came = wait_until(Duration::from_secs(10), || read().is_some()).await;
    assert!(came, "the page token file did not appear");
    read().unwrap().trim().to_string()
}

async fn wait_page(node: &Hosted) {
    let up = wait_until(Duration::from_secs(10), || {
        let (status, v) = node.get("/api/state");
        status == 200 && v["node"]["status"] != json!("starting")
    })
    .await;
    assert!(
        up,
        "the page did not come up with a running node within 10 s"
    );
}
