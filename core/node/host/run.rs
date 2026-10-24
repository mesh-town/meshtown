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
use crate::proxy::{ProxyFront, ProxySeat};
use crate::web_gui::probe;
use crate::web_gui::{
    now_unix, ConfigState, WebHandle, WebOptions, WebStartError, DEFAULT_DRAIN, TOKEN_FILE_NAME,
};

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
        let (front, seat) = match Host::seat_proxy(&run_config).await {
            Ok(seated) => seated,
            Err(e) => {
                services.stop();
                return Err(e);
            }
        };
        let config = ConfigState::from_run(&run_config);
        let host = HostSlots::with_main(ctl.clone(), config);
        host.set_proxy_seat(seat);
        host.seat(&NetSlotId::main(), &ctl, services.sources());
        let on_signal = host.clone();
        let signal_task = spawn_stop_signal(move || on_signal.stop_all_on_signal());
        let saved = SavedSlots {
            relaunch,
            sources: services.sources(),
        };
        let result = Host::with_page(run_config, ctl, role, body, host, saved).await;
        signal_task.abort();
        if let Some(front) = front {
            front.close().await;
        }
        services.stop();
        result
    }

    /// Takes the process proxy port and makes the seat that chooses the network the proxy serves.
    #[doc(hidden)]
    pub async fn seat_proxy(
        run_config: &RunConfig,
    ) -> anyhow::Result<(Option<ProxyFront>, Arc<ProxySeat>)> {
        let front = Host::bind_front(run_config).await?;
        let process = &run_config.effective_settings.process;
        let seat = Arc::new(ProxySeat::new(front.clone(), process.proxy_net.clone()));
        Ok((front, seat))
    }

    /// Takes the process proxy port; with the proxy off, takes nothing and logs why.
    async fn bind_front(run_config: &RunConfig) -> anyhow::Result<Option<ProxyFront>> {
        let process = &run_config.effective_settings.process;
        if !process.socks_enabled {
            let by_flag = run_config.cli_override_paths.contains(&"socks.enabled");
            say("socks", socks_off_line(by_flag));
            return Ok(None);
        }
        Ok(Some(ProxyFront::bind(process).await?))
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
        let token_dir = options.token_dir.clone();
        let started = crate::web_gui::start_web_page(options, &run_config, host.clone()).await;
        let handle = match started {
            Ok(handle) => Some(handle),
            Err(e) => Host::page_failed(role, &e, token_dir).await?,
        };
        let main_run = saved.relaunch.as_ref().map(|_| run_config.clone());
        output::set_multi_net(has_saved_slots(main_run.as_ref()));
        host.spawn_task(&NetSlotId::main(), body(run_config, ctl));
        if let (Some(relaunch), Some(main_run)) = (&saved.relaunch, &main_run) {
            host.set_context(SlotContext {
                main_run: main_run.clone(),
                relaunch: relaunch.clone(),
                sources: saved.sources.clone(),
            });
            host.start_saved_slots(main_run, relaunch, &saved.sources, now_unix());
            host.report_proxy_start();
            output::set_multi_net(multi_net_for(host.len()));
        }
        let result = Host::wait_slots(&host).await;
        if let Some(handle) = handle {
            handle.shutdown(DEFAULT_DRAIN).await;
        }
        result
    }

    /// The page didn't start: by role — a launch error, or a log line and the cycle without the page.
    async fn page_failed(
        role: PageRole,
        e: &WebStartError,
        token_dir: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Option<WebHandle>> {
        match role {
            PageRole::OnlyEntry => Err(entry_error(e).await),
            PageRole::Optional => {
                if let Some(err) = same_settings_node_error(e, token_dir).await {
                    return Err(err);
                }
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
                if id.carries_process() {
                    main = outcome;
                }
            }
            for task in retiring {
                task.await.ok();
            }
        }
    }
}

/// The log line about the proxy being off: the startup flag or a setting.
fn socks_off_line(by_flag: bool) -> String {
    match by_flag {
        true => "off by the --no-socks flag".to_string(),
        false => "off in settings (socks.enabled: false)".to_string(),
    }
}

/// The busy page port of a start error, if the cause is exactly that.
fn busy_port(e: &WebStartError) -> Option<u16> {
    match e {
        WebStartError::Bind { port, kind, .. } if *kind == std::io::ErrorKind::AddrInUse => {
            Some(*port)
        }
        _ => None,
    }
}

/// A blocking probe on the blocking pool; a failed task counts as "no".
async fn probe_blocking(check: impl FnOnce() -> bool + Send + 'static) -> bool {
    output::spawn_blocking(check).await.unwrap_or(false)
}

/// A launch error without a key: the page is the only way in, and it is unavailable.
async fn entry_error(e: &WebStartError) -> anyhow::Error {
    let running = match busy_port(e) {
        Some(port) => probe_blocking(move || probe::node_page_answers(port)).await,
        None => false,
    };
    entry_error_text(e, running)
}

/// A node with a key whose page port is held by a node that accepts this settings directory's token is
/// the same node already running (for example, a service): a second one must not start beside it.
async fn same_settings_node_error(
    e: &WebStartError,
    token_dir: Option<std::path::PathBuf>,
) -> Option<anyhow::Error> {
    let port = busy_port(e)?;
    let token = token_dir
        .map(|dir| std::fs::read_to_string(dir.join(TOKEN_FILE_NAME)).unwrap_or_default())?;
    let token = token.trim().to_string();
    let same = probe_blocking(move || probe::node_accepts_token(port, &token)).await;
    same.then(|| already_running_error(port))
}

/// The text of [`entry_error`]; `node_on_port` — a Mesh Town node already answers on the busy page port.
fn entry_error_text(e: &WebStartError, node_on_port: bool) -> anyhow::Error {
    match (node_on_port, e) {
        (true, WebStartError::Bind { port, .. }) => already_running_error(*port),
        _ => generic_entry_error(e),
    }
}

fn already_running_error(port: u16) -> anyhow::Error {
    let program = crate::cli::program_name::program_name();
    anyhow::anyhow!(
        "{NET_NAME}: a Mesh Town node is already running on port {port} (for example, as a service). Its page is on port {port} of this device (http://<device address>:{port}/). To get a new sign-in link, run: {program} password reset. There is no need to start a second node."
    )
}

fn generic_entry_error(e: &WebStartError) -> anyhow::Error {
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
