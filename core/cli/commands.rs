//! Execution of the console subcommands: `keygen`, `config-path`, `run`. Flag parsing is in `cli.rs`.
//! Here: building `CliOverrides` from flags, preparing the start of the config file and choosing the branch;
//! the branches themselves are `run_key` (with a key) and `run_nokey` (without a key), the page and the node are in `crate::node::host`.

use crate::config::config_file::{self, CliOverrides, ConfigSource, TransportMode};
use crate::members::network_key::{format_key_string, normalize_label, NetworkKey};
use crate::members::words::generate_tail;
use crate::node::output::NET_NAME;
use clap::parser::ValueSource;
use clap::ArgMatches;

use crate::cli::args::{is_flag_typed, toggle_from_flag_pair};

pub(crate) mod access;
pub(crate) mod ifaces;
mod node_link;
pub(crate) mod password;
mod run_args;
mod run_invite;
mod run_invite_addnet;
mod run_invite_seed;
mod run_key;
mod run_nokey;
pub(crate) use run_args::RunArgs;
use run_invite_addnet::{add_net_slot, plan_cli_key, KeyNet};
use run_key::run_with_key;
use run_nokey::run_without_key;

/// The `keygen` command: prints a new network key — with words (default) or the machine one with `--raw`.
pub(crate) fn keygen(label: Vec<String>, raw: bool) -> anyhow::Result<()> {
    if raw {
        println!("{}", NetworkKey::generate()?.encode());
        return Ok(());
    }
    let tail = generate_tail()?;
    let label = if label.is_empty() {
        None
    } else {
        Some(normalize_label(&label.join(" "))?)
    };
    println!("{}", format_key_string(label.as_deref(), &tail));
    Ok(())
}

/// The `config-path` command: prints the path to the config file and starts nothing.
pub(crate) fn config_path(config: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    match config_file::resolve_config_source(
        config.as_deref(),
        &config_file::current_env().as_path_env(),
    ) {
        ConfigSource::File { path, .. } => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigSource::Nowhere(why) => {
            anyhow::bail!("no place for the settings file: {why}")
        }
        // `config-path` doesn't know `--no-config`, so we can't get here; we don't go on silently.
        ConfigSource::Disabled => {
            anyhow::bail!("no place for the settings file: the settings file is disabled")
        }
    }
}

/// The `run` command: collects the flags into `CliOverrides`, prepares the config file and starts the node.
pub(crate) async fn run_node_command(args: RunArgs, m: &ArgMatches) -> anyhow::Result<()> {
    let overrides = build_overrides(m, &args);
    if args.no_config && is_flag_typed(m, "config") {
        anyhow::bail!("--no-config and --config together make no sense: remove one of the flags");
    }
    let invited = args
        .invite
        .as_deref()
        .map(crate::invite::parse)
        .transpose()?;
    let source = resolve_source(args.no_config, args.config.as_deref());
    let now = now_unix();
    let (run, overrides, invited) = prepare_run(&args, overrides, invited, &source, now).await?;
    match run.effective_settings.net.network_key.clone() {
        Some(key_text) => {
            run_with_key(
                key_text,
                overrides,
                run,
                args.initial_domains,
                args.blocked_domains,
                invited,
            )
            .await
        }
        None if args.no_config => {
            anyhow::bail!(
                "the {NET_NAME} network key is not set: provide it with --key, the MESHTOWN_KEY variable or the network.key field of {}, or join with an invite: --invite \"<invite>\". A new key is printed by: {cmd} keygen <label>, for example {cmd} keygen our network",
                file_hint(&run.file_report),
                cmd = crate::cli::program_name::program_name()
            )
        }
        None => run_without_key(overrides, run, args.initial_domains, args.blocked_domains).await,
    }
}

/// Prepares the main run: the settings file, the invite and `--key` of another network (each becomes a slot).
/// Returns the run, the flags for the main network and the invite that belongs to the main network.
async fn prepare_run(
    args: &RunArgs,
    overrides: CliOverrides,
    invited: Option<crate::invite::ParsedInvite>,
    source: &ConfigSource,
    now: u64,
) -> anyhow::Result<(
    config_file::RunConfig,
    CliOverrides,
    Option<crate::invite::ParsedInvite>,
)> {
    let cli_key = overrides.network_key.clone();
    let key_net = plan_cli_key(source, cli_key.as_deref())?;
    let overrides = overrides_for_main(&overrides, &key_net);
    let mut run = config_file::prepare(source, &overrides, args.save_config, now)?;
    let typed = &args.initial_domains;
    let invited =
        run_invite::settle_invite(invited, cli_key.as_deref(), &mut run, typed, now).await?;
    let added = add_key_net(&key_net, cli_key.as_deref(), &run, args, now)?;
    Ok((run, overrides, invited.filter(|_| !added)))
}

/// The flags for the main network: a key of another network is not the main one, it goes to a slot.
fn overrides_for_main(overrides: &CliOverrides, key_net: &KeyNet) -> CliOverrides {
    let mut main = overrides.clone();
    if matches!(key_net, KeyNet::Add | KeyNet::Existing) {
        main.network_key = None;
    }
    main
}

/// `--key` of another network: saves it as a network slot (one that is already saved is left as it is).
/// Returns whether the key belongs to a slot: then the invite's label and addresses are not for the main network.
fn add_key_net(
    key_net: &KeyNet,
    cli_key: Option<&str>,
    run: &config_file::RunConfig,
    args: &RunArgs,
    now: u64,
) -> anyhow::Result<bool> {
    if let (KeyNet::Add, Some(key)) = (key_net, cli_key) {
        add_net_slot(run, key, &args.initial_domains, &[], now)?;
    }
    Ok(matches!(key_net, KeyNet::Add | KeyNet::Existing))
}

/// Collects the `run` subcommand flags into `CliOverrides`: what the person didn't type stays `None`
/// (then the config file or the default decides), the `--x/--no-x` pairs are handled by `toggle_from_flag_pair`.
fn build_overrides(m: &ArgMatches, args: &RunArgs) -> CliOverrides {
    CliOverrides {
        network_key: args.key.clone().filter(|_| {
            matches!(
                m.value_source("key"),
                Some(ValueSource::CommandLine | ValueSource::EnvVariable)
            )
        }),
        transport_mode: if args.relay_only {
            Some(TransportMode::RelayOnly)
        } else if args.no_relay {
            Some(TransportMode::NoRelay)
        } else {
            None
        },
        exit_enabled: toggle_from_flag_pair(args.exit, args.no_exit),
        socks_enabled: toggle_from_flag_pair(args.socks, args.no_socks),
        socks_port: args.socks_port.filter(|_| is_flag_typed(m, "socks_port")),
        socks_access: args
            .socks_access
            .filter(|_| is_flag_typed(m, "socks_access")),
        checks_enabled: toggle_from_flag_pair(args.checks, args.no_checks),
        check_interval_sec: args
            .check_interval
            .filter(|_| is_flag_typed(m, "check_interval")),
        doh_server: args.check_doh.filter(|_| is_flag_typed(m, "check_doh")),
        provider_dns: args
            .provider_dns
            .filter(|_| is_flag_typed(m, "provider_dns")),
        probe_interval_sec: args
            .probe_interval
            .filter(|_| is_flag_typed(m, "probe_interval")),
        probe_full_cycle: args.probe_full_cycle.then_some(true),
        web_port: args.web_port.filter(|_| is_flag_typed(m, "web_port")),
    }
}

/// The source of the config file: disabled (`--no-config`) or the usual lookup by path/default.
fn resolve_source(no_config: bool, config: Option<&std::path::Path>) -> ConfigSource {
    if no_config {
        ConfigSource::Disabled
    } else {
        config_file::resolve_config_source(config, &config_file::current_env().as_path_env())
    }
}

/// The current time in seconds of the Unix epoch; with a broken clock — 0 (as before).
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where the config file came from — for the error text: a path or "the settings file (not used)".
fn file_hint(r: &config_file::FileReport) -> String {
    match &r.path {
        Some(p) => p.display().to_string(),
        None => "the settings file (not used)".to_string(),
    }
}
