//! A node without a network under the process host, on top of `common/nets_host.rs`.
//! Included as `#[path = "common/nets_host_empty.rs"] mod nets_host_empty;` next to `mod nets_host;`.

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule};
use meshtown_core::node::host::{Host, PageRole};
use meshtown_core::node::node_ctl::{NodeCtl, Relaunch};
use meshtown_core::node::{self, output, IdentityState};

use super::nets_host::{free_port, scratch_dir, wait_page, wait_token, HostNode};

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
