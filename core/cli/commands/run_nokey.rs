//! Starting the node without a network key: the node page is the only way into the network.
//! What is not here: choosing the launch branch and parsing flags — that is in `commands.rs`; the branch with a key is in
//! `run_key.rs`. The page and its shutdown are handled by `crate::node::host`.

use crate::config::config_file::{CliOverrides, RunConfig};
use crate::node::host::{Host, PageRole};
use crate::node::output::{say, NET_NAME};

use super::run_key::build_relaunch;

/// Two hint lines after the page starts successfully: how to join the network.
fn hint_lines() {
    say(
        "node",
        format!(
            "You haven't joined a {NET_NAME} network yet. Open the node page (its address is in the \"node page\" line above) and click \"Join with an invite\" or \"Create a new network\"."
        ),
    );
    say(
        "node",
        format!(
            "Got an invite? You can also join from the terminal: press Ctrl+C, then run {} run --invite \"<invite>\"",
            crate::cli::program_name::program_name()
        ),
    );
}

/// The branch without a network key (except `--no-config`): starts the node page and waits until the person
/// joins the network by invite or creates a new one on the page.
pub(super) async fn run_without_key(
    overrides: CliOverrides,
    run: RunConfig,
    initial_domains: Vec<String>,
    blocked_domains: Vec<String>,
) -> anyhow::Result<()> {
    let relaunch = build_relaunch(&run, overrides, initial_domains, blocked_domains);
    let ctl = crate::node::node_ctl::NodeCtl::new_without_network(&run.effective_settings);
    let saved = Some(relaunch.clone());
    Host::run_with_saved_slots(
        run,
        ctl,
        PageRole::OnlyEntry,
        saved,
        |run, ctl| async move {
            hint_lines();
            crate::node::run_network(None, run, ctl, relaunch).await
        },
    )
    .await
}
