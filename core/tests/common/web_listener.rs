//! Shared scaffolding of the `tests/web_listener_*.rs` scenarios: bringing up the listener (modules 01–04) on port 0
//! (`start_web`) with the fake node `TestNode` (it lives in `test_node.rs` and is re-exported from here), the
//! scratch directory and the run config. Each scenario includes it as
//! `#[path = "common/web_listener.rs"] mod common;`. What only some scenarios need lives in leaf files next to it
//! (`web_listener_resp.rs` — the raw HTTP client `http_within` and the response parser `Resp`; on top of it
//! `web_listener_http.rs`, `web_listener_api.rs`, `web_listener_headers.rs`, `web_listener_stream.rs`), which a scenario
//! includes as its own modules; they take this base through `super::common` and the client through `super::resp_common`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule, RunConfig};
use meshtown_core::web_gui::{self, WebHandle, WebOptions, DEFAULT_DRAIN};

#[path = "test_node.rs"]
mod test_node;
pub(crate) use test_node::TestNode;

// ============================================================================================
// Scaffolding
// ============================================================================================

/// The test's temporary directory: `$TMPDIR/mesh-web-<tag>-<pid>-<nanos>`, already created.
pub(crate) fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("mesh-web-{tag}-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).expect("scratch: creating the temporary directory");
    dir
}

/// Restores the permissions of the directory and all files in it to `0755`/`0644` (if the test changed them) and removes
/// the directory entirely. Call at the end of every test.
pub(crate) fn cleanup(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ =
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o644));
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `config_file::prepare` with an explicit `ConfigSource::File` path in `dir` — as in the page listener's test scaffolding.
pub(crate) fn run_config(dir: &Path) -> RunConfig {
    let mut run = config_file::prepare(
        &ConfigSource::File {
            path: dir.join("config.json"),
            may_create: true,
            rule: PathRule::Explicit,
        },
        &CliOverrides::default(),
        true,
        0,
    )
    .expect("run_config: prepare");
    // Test listeners aren't opened to the local network (the macOS firewall prompt).
    run.effective_settings.process.web_access = config_file::ListenAccess::Localhost;
    run.file_settings.process.web_access = config_file::ListenAccess::Localhost;
    run
}

/// Brings up a listener on port 0 with `TestNode`, the token directory being `dir`.
pub(crate) async fn start_web(dir: &Path, run: &RunConfig) -> (WebHandle, Arc<TestNode>) {
    let node = Arc::new(TestNode::new());
    let opts = WebOptions {
        enabled: true,
        port: 0,
        token_dir: Some(dir.to_path_buf()),
        fixed_token: None,
        drain: DEFAULT_DRAIN,
    };
    let handle = web_gui::start_web_page(opts, run, host_of(node.clone(), run))
        .await
        .expect("start_web: web_gui::start must come up");
    (handle, node)
}

/// A host with one `main` slot around `node`, with the config state of `run`.
pub(crate) fn host_of(
    node: Arc<dyn web_gui::NodeControl>,
    run: &RunConfig,
) -> Arc<dyn web_gui::HostControl> {
    web_gui::single_slot_host(node, web_gui::ConfigState::from_run(run))
}
