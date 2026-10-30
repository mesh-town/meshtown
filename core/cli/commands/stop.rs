//! The `stop` subcommand: stops the running node and waits until it is gone.

use std::time::{Duration, Instant};

use super::client::Node;
use crate::cli::args::Globals;
use crate::cli::exit::NotRunning;

/// How long the node gets to go away after the request.
const STOP_LIMIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(200);

/// `stop` has no options of its own.
#[derive(clap::Args)]
pub(crate) struct StopArgs {}

pub(crate) async fn run(args: StopArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("stop", &[])?;
    let _ = args;
    let node = Node::connect(g)?;
    node.call("POST", "/api/actions/stop", None)?;
    wait_gone(&node).await?;
    eprintln!("Mesh Town stopped.");
    Ok(())
}

/// Polls the page until nothing answers on its port.
async fn wait_gone(node: &Node) -> anyhow::Result<()> {
    let deadline = Instant::now() + STOP_LIMIT;
    while Instant::now() < deadline {
        if is_gone(&node.call("GET", "/api/nets", None)) {
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }
    anyhow::bail!(
        "Mesh Town is still stopping after {} s",
        STOP_LIMIT.as_secs()
    )
}

fn is_gone(answer: &anyhow::Result<serde_json::Value>) -> bool {
    matches!(answer, Err(e) if e.chain().any(|c| c.is::<NotRunning>()))
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
