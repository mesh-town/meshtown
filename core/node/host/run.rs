//! The assembly point "log capacity → process services → stop signal → page → body → page shutdown".
//! What isn't here: flag parsing, `Relaunch` and `NodeConfig` are in `commands`; the node itself is in `node`.
//! The memory guard and the stop signal exist once per process, so they live here and not in the node.
//! Related: `web` (the node page), `node_ctl` (the shared `NodeCtl`), `output` (the log).

use std::future::Future;
use std::sync::Arc;

use super::process::{heap_dump_dir, ProcessServices};
use super::saved_slots::{has_saved_slots, multi_net_for};
use super::signal::spawn_stop_signal;
use super::slots::{HostSlots, SlotContext};
use crate::config::config_file::{NetSlotId, RunConfig};
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output::{self, say, NET_NAME};
use crate::web_gui::{now_unix, ConfigState, WebHandle, WebOptions, WebStartError, DEFAULT_DRAIN};

/// The role of the node page in a launch: what to do if it didn't start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageRole {
    /// A node with a key: if the page didn't start — a log line, and the node goes on without it.
    Optional,
    /// A node without a key: the page is the only way into the network; if it didn't start — a launch error.
    OnlyEntry,
}

/// What the main slot hands to the start of the saved slots.
struct SavedSlots {
    relaunch: Option<Relaunch>,
    sources: GuardSources,
}

/// The process host. It has no state: a single function `run_with_web_page`.
pub struct Host;

impl Host {
    /// Sets the log capacity, starts the process services, the slot map and the stop signal, starts the
    /// page, runs `body` as the main slot's cycle, then stops the page, the signal and the services.
    /// `body` receives the `RunConfig` and the same `NodeCtl` that went to the page.
    pub async fn run_with_web_page<B, F>(
        run_config: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        body: B,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F + Send + 'static,
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        Host::run_with_saved_slots(run_config, ctl, role, None, body).await
    }

    /// Like `run_with_web_page`; with `relaunch` the networks saved under `nets/` are started beside the
    /// main slot (only when the run has a settings file).
    pub async fn run_with_saved_slots<B, F>(
        run_config: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        relaunch: Option<Relaunch>,
        body: B,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F + Send + 'static,
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        crate::node::output::set_log_capacity(
            run_config.effective_settings.process.log_ring_lines as usize,
        );
        let dump_dir = heap_dump_dir(run_config.file_report.path.as_deref());
        let services = ProcessServices::start(&run_config.effective_settings.process, dump_dir);
        ctl.set_guard_sources(NetSlotId::main(), services.sources());
        let config = ConfigState::from_run(&run_config);
        let host = HostSlots::with_main(ctl.clone(), config);
        let on_signal = host.clone();
        let signal_task = spawn_stop_signal(move || on_signal.stop_all_on_signal());
        let saved = SavedSlots {
            relaunch,
            sources: services.sources(),
        };
        let result = Host::with_page(run_config, ctl, role, body, host, saved).await;
        signal_task.abort();
        services.stop();
        result
    }

    /// Starts the page and runs the main slot's cycle around it; the page is stopped after the slots.
    async fn with_page<B, F>(
        run_config: RunConfig,
        ctl: Arc<NodeCtl>,
        role: PageRole,
        body: B,
        host: Arc<HostSlots>,
        saved: SavedSlots,
    ) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F + Send + 'static,
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let options = WebOptions::for_run(&run_config);
        let started = crate::web_gui::start_web_page(options, &run_config, host.clone()).await;
        let handle = match started {
            Ok(handle) => Some(handle),
            Err(e) => Host::page_failed(role, &e)?,
        };
        let main_run = saved.relaunch.as_ref().map(|_| run_config.clone());
        output::set_multi_net(has_saved_slots(main_run.as_ref()));
        let labeled = output::with_net(Some("main".into()), body(run_config, ctl));
        let task = output::spawn(labeled);
        host.set_task(&NetSlotId::main(), task);
        if let (Some(relaunch), Some(main_run)) = (&saved.relaunch, &main_run) {
            host.set_context(SlotContext {
                main_run: main_run.clone(),
                relaunch: relaunch.clone(),
                sources: saved.sources.clone(),
            });
            host.start_saved_slots(main_run, relaunch, &saved.sources, now_unix());
            output::set_multi_net(multi_net_for(host.len()));
        }
        let result = Host::wait_slots(&host).await;
        if let Some(handle) = handle {
            handle.shutdown(DEFAULT_DRAIN).await;
        }
        result
    }

    /// The page didn't start: by role — a launch error, or a log line and the cycle without the page.
    fn page_failed(role: PageRole, e: &WebStartError) -> anyhow::Result<Option<WebHandle>> {
        match role {
            PageRole::OnlyEntry => Err(entry_error(e)),
            PageRole::Optional => {
                e.continue_without_page_log_line()
                    .into_iter()
                    .for_each(|line| say("web", line));
                Ok(None)
            }
        }
    }

    /// Waits for every slot's task; the outcome is the main slot's, a panicked task is an error.
    async fn wait_slots(host: &HostSlots) -> anyhow::Result<()> {
        let mut main = Ok(());
        loop {
            let tasks = host.take_tasks();
            let retiring = host.take_retiring();
            if tasks.is_empty() && retiring.is_empty() {
                return main;
            }
            for (id, task) in tasks {
                let outcome = task.await.unwrap_or_else(|e| {
                    Err(anyhow::anyhow!("the node task of slot {id} failed: {e}"))
                });
                if id == NetSlotId::main() {
                    main = outcome;
                }
            }
            for task in retiring {
                task.await.ok();
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
