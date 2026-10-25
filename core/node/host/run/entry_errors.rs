//! Launch errors of the process host: a second copy on the page port and an unavailable page of a node without a key.

use super::{busy_port, probe_blocking};
use crate::config::config_file::RunConfig;
use crate::node::output::NET_NAME;
use crate::web_gui::probe;
use crate::web_gui::{WebOptions, WebStartError, TOKEN_FILE_NAME};

/// A launch error without a key: the page is the only way in, and it is unavailable.
pub(super) async fn entry_error(e: &WebStartError) -> anyhow::Error {
    let running = match busy_port(e) {
        Some(port) => probe_blocking(move || probe::node_page_answers(port)).await,
        None => false,
    };
    entry_error_text(e, running)
}

/// Before any port is taken: with the page on, a node that accepts this settings directory's token on the page
/// port is the same node already running (for example, a service) - a second one must not start beside it.
pub(super) async fn refuse_second_node(run_config: &RunConfig) -> anyhow::Result<()> {
    let process = &run_config.effective_settings.process;
    if !process.web_enabled {
        return Ok(());
    }
    let token_dir = WebOptions::for_run(run_config).token_dir;
    match same_settings_node(process.web_port, token_dir).await {
        true => Err(already_running_error(process.web_port)),
        false => Ok(()),
    }
}

/// A node with a key whose page port is held by a node that accepts this settings directory's token.
pub(super) async fn same_settings_node_error(
    e: &WebStartError,
    token_dir: Option<std::path::PathBuf>,
) -> Option<anyhow::Error> {
    let port = busy_port(e)?;
    same_settings_node(port, token_dir)
        .await
        .then(|| already_running_error(port))
}

/// Whether the node on the page port accepts the permanent token from `web-token` in `token_dir`.
async fn same_settings_node(port: u16, token_dir: Option<std::path::PathBuf>) -> bool {
    let Some(dir) = token_dir else {
        return false;
    };
    let text = std::fs::read_to_string(dir.join(TOKEN_FILE_NAME)).unwrap_or_default();
    let token = text.trim().to_string();
    probe_blocking(move || probe::node_accepts_token(port, &token)).await
}

/// The text of [`entry_error`]; `node_on_port` — a Mesh Town node already answers on the busy page port.
pub(super) fn entry_error_text(e: &WebStartError, node_on_port: bool) -> anyhow::Error {
    match (node_on_port, e) {
        (true, WebStartError::Bind { port, .. }) => already_running_error(*port),
        _ => generic_entry_error(e),
    }
}

fn already_running_error(port: u16) -> anyhow::Error {
    let program = crate::cli::program_name::program_name();
    anyhow::anyhow!(
        "{NET_NAME}: another copy is already running on port {port} (for example, as a service). Its page is on port {port} of this device (http://<device address>:{port}/). To get a new sign-in link, run: {program} password reset. There is no need to start a second copy."
    )
}

fn generic_entry_error(e: &WebStartError) -> anyhow::Error {
    let advice = e
        .start_failure_advice()
        .map(|a| format!(" {a}"))
        .unwrap_or_default();
    anyhow::anyhow!(
        "{NET_NAME}: the page is unavailable, and without a network key you can join the network only through it. Reason: {e}.{advice} Or join the network by starting with an invite: {cmd} run --invite \"<invite>\" (network key — --key or MESHTOWN_KEY)",
        cmd = crate::cli::program_name::program_name()
    )
}
