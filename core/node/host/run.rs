//! The assembly point "log capacity → process services → stop signal → page → body → page shutdown".
//! What isn't here: flag parsing, `Relaunch` and `NodeConfig` are in `commands`; the node itself is in `node`.
//! The memory guard and the stop signal exist once per process, so they live here and not in the node.
//! Related: `web` (the node page), `node_ctl` (the shared `NodeCtl`), `output` (the log). Later a list
//! of slots (several networks in one process) will go here.

use std::future::Future;
use std::sync::Arc;

use super::process::{heap_dump_dir, ProcessServices};
use super::signal::spawn_stop_signal;
use crate::config::config_file::RunConfig;
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{say, NET_NAME};
use crate::web_gui::{WebOptions, WebStartError, DEFAULT_DRAIN};

/// The role of the node page in a launch: what to do if it didn't start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageRole {
    /// A node with a key: if the page didn't start — a log line, and the node goes on without it.
    Optional,
    /// A node without a key: the page is the only way into the network; if it didn't start — a launch error.
    OnlyEntry,
}

/// The process host. It has no state: a single function `run_with_web_page`.
pub struct Host;

impl Host {
    /// Sets the log capacity, starts the process services and the stop signal, starts the page, runs
    /// `body`, then stops the page, the signal and the services.
    /// `body` receives the `RunConfig` and the same `NodeCtl` that went to the page.
    pub async fn run_with_web_page<B, F>(
        run_config: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        body: B,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F,
        F: Future<Output = anyhow::Result<()>>,
    {
        crate::node::output::set_log_capacity(
            run_config.effective_settings.process.log_ring_lines as usize,
        );
        let dump_dir = heap_dump_dir(run_config.file_report.path.as_deref());
        let services = ProcessServices::start(&run_config.effective_settings.process, dump_dir);
        ctl.set_guard_sources(services.sources());
        let signal_task = spawn_stop_signal(ctl.clone());
        let result = Host::with_page(run_config, ctl, role, body).await;
        signal_task.abort();
        services.stop();
        result
    }

    /// Starts the page and runs `body` around it; the page is stopped after `body`.
    async fn with_page<B, F>(
        run_config: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        body: B,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F,
        F: Future<Output = anyhow::Result<()>>,
    {
        let options = WebOptions::for_run(&run_config);
        match crate::web_gui::start_web_page(options, &run_config, ctl.clone()).await {
            Ok(handle) => {
                let result = body(run_config, ctl).await;
                handle.shutdown(DEFAULT_DRAIN).await;
                result
            }
            Err(e) => Host::without_page(run_config, ctl, role, e, body).await,
        }
    }

    /// The page didn't start: by role — a launch error, or a log line and the body without the page.
    async fn without_page<B, F>(
        run: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        e: WebStartError,
        body: B,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F,
        F: Future<Output = anyhow::Result<()>>,
    {
        match role {
            PageRole::OnlyEntry => Err(entry_error(&e)),
            PageRole::Optional => {
                if let Some(line) = e.continue_without_page_log_line() {
                    say("web", line);
                }
                body(run, ctl).await
            }
        }
    }
}

/// A launch error without a key: the page is the only way in, and it is unavailable.
fn entry_error(e: &WebStartError) -> anyhow::Error {
    let advice = e
        .start_failure_advice()
        .map(|a| format!(" {a}"))
        .unwrap_or_default();
    anyhow::anyhow!(
        "{NET_NAME}: the node page is unavailable, and without a network key you can join the network only through it. Reason: {e}.{advice} Or join the network by starting with an invite: {cmd} run --invite \"<invite>\" (network key — --key or MESHTOWN_KEY)",
        cmd = crate::cli::program_name::program_name()
    )
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
