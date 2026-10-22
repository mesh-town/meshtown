//! Page demo node: the real `meshtown::web::start_web_page` on top of a fake node
//! `DemoNode`, which drives model time on its own and fakes plausible snapshot data.
//!
//! Why: manual checking of the page in a browser, and the
//! `make web-check` run all need a real binary with a real HTTP stack, not
//! tests inside the cargo process.
//! Non-goals: does not touch a real node — the whole node here is a stage prop.
//! Related: `docs/WEB-API.md` is the specification of the flags, the settings-file
//! preparation procedures and the `DemoNode` behavior.
//! `docs/WEB-API.md`.
//!
//! Files: `args.rs` — CLI flags; `overrides.rs` — parsing of `--cli-override`; `config.rs` —
//! preparation of the settings file; `rng.rs` — deterministic generator; `node.rs` — the state of
//! `DemoNode`; `background.rs` — background model-time task; `ports.rs` — the `ports` section of the
//! snapshot; `members.rs` and `members_reply.rs` — the member list and its actions; `snapshot.rs` — assembling `NodeSnapshot`; `control.rs` — `impl NodeControl`; `shutdown.rs` — shutdown and cleanup.

mod args;
mod background;
mod config;
mod control;
mod members;
mod members_reply;
mod node;
mod overrides;
mod ports;
mod rng;
mod shutdown;
mod snapshot;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;

use meshtown::config_file;
use meshtown::web::{self, WebOptions, DEFAULT_DRAIN};

use crate::args::Args;
use crate::background::run_background;
use crate::config::prepare_demo_config;
use crate::node::{DemoNetworkOptions, DemoNode};
use crate::shutdown::{cleanup_dir, wait_for_sigterm};

fn current_unix_time() -> anyhow::Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

fn build_demo_node(args: &Args, run: &config_file::RunConfig) -> DemoNode {
    let seed = if args.seed == 0 { 1 } else { args.seed };
    let ring_capacity = args
        .ring_lines
        .unwrap_or(run.effective_settings.process.log_ring_lines as usize)
        .max(1);
    DemoNode::new(
        seed,
        args.fast,
        ring_capacity,
        args.no_network,
        DemoNetworkOptions {
            offline_peers: args.offline_peers,
            dht_joined: args.dht_joined,
            dht_nodes: args.dht_nodes,
            members_second_vote: args.members_second_vote,
            me_excluded: args.me_excluded,
            members_pending: args.members_pending,
            me_pending: args.me_pending,
        },
        run.effective_settings.clone(),
    )
}

/// `--no-web` — only prints a message; otherwise starts the page and prints its address.
async fn start_web_if_enabled(
    args: &Args,
    run: &config_file::RunConfig,
    demo: Arc<DemoNode>,
) -> Option<web::WebHandle> {
    if args.no_web {
        println!("page: not started (--no-web)");
        return None;
    }
    let mut opts = WebOptions::for_run(run);
    opts.port = args.port;
    opts.fixed_token = args.token.clone();
    match web::start_web_page(opts, run, demo).await {
        Ok(h) => {
            println!("page: http://127.0.0.1:{}/", h.shared.port());
            Some(h)
        }
        Err(e) => {
            eprintln!("web_demo: couldn't start the page: {e}");
            std::process::exit(1);
        }
    }
}

async fn wait_for_shutdown(demo: &DemoNode) {
    tokio::select! {
        _ = demo.notify.notified() => {}
        _ = tokio::signal::ctrl_c() => {}
        _ = wait_for_sigterm() => {}
    }
}

async fn shutdown_demo(
    bg: tokio::task::JoinHandle<()>,
    handle: Option<web::WebHandle>,
    dir: Option<PathBuf>,
) {
    bg.abort();
    if let Some(h) = handle {
        h.shutdown(DEFAULT_DRAIN).await;
    }
    if let Some(dir) = dir {
        cleanup_dir(&dir);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let now = current_unix_time()?;
    let (run, dir) = prepare_demo_config(&args, now)?;

    let demo = Arc::new(build_demo_node(&args, &run));
    let handle = start_web_if_enabled(&args, &run, demo.clone()).await;
    let bg = tokio::spawn(run_background(demo.clone()));

    wait_for_shutdown(&demo).await;

    shutdown_demo(bg, handle, dir).await;
    Ok(())
}
