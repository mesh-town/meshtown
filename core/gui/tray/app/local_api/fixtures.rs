//! A fake node and shared helpers for the `local_api` tests. The file is called `fixtures.rs`, but
//! `mod.rs` declares it as the module `tests` (`#[path = "fixtures.rs"] pub(crate) mod tests;`), so that
//! the path `crate::gui::tray::app::local_api::tests::…` does not change — the `tray_role` module reads it too (
//! `poller_own_tests.rs`, `poller_misc_tests.rs`, `poller_foreign_tests.rs`, `run_tests.rs`).

use portable_atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A token that is clearly not a real one, for tests and the fake node: 52 characters `[0-9A-Za-z]`.
pub(crate) const FAKE_TOKEN: &str = "FAKEnodeABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqr";

/// A fresh unique directory in `temp_dir()`. A counter on top of nanoseconds, as in `web/token.rs`: on
/// macOS the clock resolution is coarser than a nanosecond, so parallel tests could collide on the same name.
pub(crate) fn temp_dir(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "meshtown-tray-local-api-{tag}-{}-{}-{}",
        std::process::id(),
        nanos,
        seq
    ));
    std::fs::create_dir_all(&dir).expect("temp_dir: create a temporary directory");
    dir
}

/// The fake node: `snapshot` is `Default` with status `online`, `stop` sets a flag, the other
/// methods are empty successful replies (modelled on `TestNode`, `tests/web_listener.rs:84–224`).
pub(crate) struct FakeNode {
    stopped: AtomicBool,
    /// The string for `GET /api/network/invite`; `None` means the node answers 409 (still joining the network).
    invite: Mutex<Option<String>>,
    /// The `exit.enabled` value of the snapshot; starts as in the default settings (`true`), and `apply_config`
    /// rewrites it from the accepted settings.
    exit_enabled: AtomicBool,
}

impl FakeNode {
    fn new() -> Self {
        FakeNode {
            stopped: AtomicBool::new(false),
            invite: Mutex::new(None),
            exit_enabled: AtomicBool::new(true),
        }
    }

    /// Sets the fake's invite: `Some` is served, `None` gives 409.
    pub(crate) fn set_invite(&self, text: Option<&str>) {
        *self.invite.lock().unwrap() = text.map(str::to_string);
    }

    pub(crate) fn stop_called(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

impl crate::web_gui::NodeControl for FakeNode {
    fn snapshot(&self) -> crate::web_gui::NodeSnapshot {
        crate::web_gui::NodeSnapshot {
            node: crate::web_gui::NodeRuntime {
                status: crate::web_gui::NodeStatus::Online,
                ..Default::default()
            },
            exit: crate::web_gui::ExitInfo {
                enabled: self.exit_enabled.load(Ordering::SeqCst),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn log_after(&self, _after: u64, _limit: usize) -> crate::web_gui::LogPage {
        crate::web_gui::LogPage::default()
    }

    fn find_exit(&self, _domain: &str) -> Result<(), crate::web_gui::ActionError> {
        Ok(())
    }

    fn check_domain(&self, _domain: &str) -> Result<(), crate::web_gui::ActionError> {
        Ok(())
    }

    fn check_all(&self) -> Result<u64, crate::web_gui::ActionError> {
        Ok(0)
    }

    fn add_domain(&self, domain: &str) -> Result<String, crate::web_gui::ActionError> {
        Ok(domain.to_string())
    }

    fn remove_domain(&self, domain: &str) -> Result<String, crate::web_gui::ActionError> {
        Ok(domain.to_string())
    }

    fn add_peer_address(&self, _line: &str) -> Result<(), String> {
        Ok(())
    }

    fn apply_config(
        &self,
        settings: &crate::config::config_file::Settings,
        _paths: &[&'static str],
    ) {
        self.exit_enabled
            .store(settings.net.exit_enabled, Ordering::SeqCst);
    }

    fn switch_network(&self, _key: &str) {}

    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    fn invite(&self) -> Result<String, crate::web_gui::ActionError> {
        match self.invite.lock().unwrap().clone() {
            Some(text) => Ok(text),
            None => Err(crate::web_gui::ActionError::Busy(
                "the node is still joining the network".to_string(),
            )),
        }
    }
}

/// Starts a real `web::start_web_page` on `port` (0 means any free one) with `FakeNode` and the token
/// `FAKE_TOKEN`; the token directory is `token_dir`. A `web::start_web_page` error is a `panic!`.
pub(crate) async fn start_fake(
    port: u16,
    token_dir: &std::path::Path,
) -> (crate::web_gui::WebHandle, Arc<FakeNode>) {
    let node = Arc::new(FakeNode::new());
    let run = crate::config::config_file::prepare(
        &crate::config::config_file::ConfigSource::Disabled,
        &crate::config::config_file::CliOverrides::default(),
        false,
        0,
    )
    .expect("start_fake: prepare with ConfigSource::Disabled doesn't fail");
    let opts = crate::web_gui::WebOptions {
        enabled: true,
        port,
        token_dir: Some(token_dir.to_path_buf()),
        fixed_token: Some(FAKE_TOKEN.to_string()),
        drain: crate::web_gui::DEFAULT_DRAIN,
    };
    let handle = crate::web_gui::start_web_page(opts, &run, host_of(node.clone(), &run))
        .await
        .unwrap_or_else(|e| panic!("start_fake: web_gui::start must come up: {e}"));
    (handle, node)
}

/// Like `start_fake`, but with a real config file (`token_dir/config.json`, created on the fly):
/// `PUT /api/config` under a bare `start_fake` is always 500 — it has `ConfigSource::Disabled`, so there is
/// nowhere to write (see `web::config_api::put::writable_path`). Needed by tests that check a real
/// write through the HTTP loop, not just body parsing.
pub(crate) async fn start_fake_with_config(
    port: u16,
    token_dir: &std::path::Path,
) -> (crate::web_gui::WebHandle, Arc<FakeNode>) {
    let node = Arc::new(FakeNode::new());
    let source = crate::config::config_file::ConfigSource::File {
        path: token_dir.join("config.json"),
        may_create: true,
        rule: crate::config::config_file::PathRule::Explicit,
    };
    let run = crate::config::config_file::prepare(
        &source,
        &crate::config::config_file::CliOverrides::default(),
        false,
        0,
    )
    .expect("start_fake_with_config: prepare with ConfigSource::File doesn't fail");
    let opts = crate::web_gui::WebOptions {
        enabled: true,
        port,
        token_dir: Some(token_dir.to_path_buf()),
        fixed_token: Some(FAKE_TOKEN.to_string()),
        drain: crate::web_gui::DEFAULT_DRAIN,
    };
    let handle = crate::web_gui::start_web_page(opts, &run, host_of(node.clone(), &run))
        .await
        .unwrap_or_else(|e| panic!("start_fake_with_config: web_gui::start must come up: {e}"));
    (handle, node)
}

/// A host with one `main` slot around `node`, with the config state of `run`.
fn host_of(
    node: Arc<dyn crate::web_gui::NodeControl>,
    run: &crate::config::config_file::RunConfig,
) -> Arc<dyn crate::web_gui::HostControl> {
    crate::web_gui::single_slot_host(node, crate::web_gui::ConfigState::from_run(run))
}
