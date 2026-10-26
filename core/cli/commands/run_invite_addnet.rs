//! Adding another network to the node from `run`: the key (or the invite) of a network that is not the main one
//! becomes a network slot under `nets/` beside the main settings file. The main file is left alone.
//! Here: the decisions about `--key`, the slot files on disk, and the site list the new slot is seeded with.

use std::path::Path;

use crate::config::config_file::{
    self, list_net_slots, load_net_file, net_slot_path, new_net_slot, save_invited_peers,
    save_net_file, ConfigSource, LoadedConfigFile, LoadedNetFile, NetSlotId, RunConfig,
};
use crate::members::network_key::parse_user_input;
use crate::members::sync::replica::entry::WireEntry;
use crate::members::sync::replica::file::{self as replica_file, Loaded};
use crate::node::host::MAX_NETS;
use crate::node::output::say;
use crate::proxy::domains::{domain_from_key, LIST_PREFIX};
use crate::transport::net::{NodeSecret, PeerAddr};

use super::run_invite::save_text;
use super::run_invite_seed::{seed_domains, seed_peers_cache};

/// What `--key` means next to the settings file and the network slots on disk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeyNet {
    /// The main network: no key in the file yet, or the same network as the file's.
    Main,
    /// Another network that already has a slot: nothing to add.
    Existing,
    /// Another network without a slot: it becomes one.
    Add,
    /// Another network, but the node already holds the most networks it can.
    TooMany,
    /// Another network's key that does not parse: no slot is added.
    Unreadable,
}

/// Whether the two key texts are of the same network; an unparsable key is never the same.
pub(crate) fn same_key_network(a: &str, b: &str) -> bool {
    match (parse_user_input(a), parse_user_input(b)) {
        (Ok(a), Ok(b)) => a.key.subkeys().network_id == b.key.subkeys().network_id,
        _ => false,
    }
}

/// Whether the network of `key` is among the slot keys.
pub(crate) fn in_slots(key: &str, slot_keys: &[Option<String>]) -> bool {
    slot_keys.iter().flatten().any(|k| same_key_network(key, k))
}

/// Whether one more network does not fit.
pub(crate) fn net_limit_reached(slot_keys: &[Option<String>]) -> bool {
    1 + slot_keys.len() >= MAX_NETS
}

/// The fate of a key typed with `--key` (or `MESHTOWN_KEY`), before any write.
pub(crate) fn plan_key_net(
    cli_key: Option<&str>,
    file_key: Option<&str>,
    slot_keys: &[Option<String>],
) -> KeyNet {
    let (Some(cli), Some(file)) = (cli_key, file_key) else {
        return KeyNet::Main;
    };
    if same_key_network(cli, file) {
        return KeyNet::Main;
    }
    if parse_user_input(cli).is_err() {
        return KeyNet::Unreadable;
    }
    if in_slots(cli, slot_keys) {
        return KeyNet::Existing;
    }
    if net_limit_reached(slot_keys) {
        return KeyNet::TooMany;
    }
    KeyNet::Add
}

/// Reads the settings file and the slots on disk and decides what `--key` means. Nothing is written.
pub(crate) fn plan_cli_key(source: &ConfigSource, cli_key: Option<&str>) -> anyhow::Result<KeyNet> {
    let ConfigSource::File { path, .. } = source else {
        return Ok(KeyNet::Main);
    };
    let slot_keys = saved_slot_keys(path);
    let net = plan_key_net(cli_key, peek_file_key(path).as_deref(), &slot_keys);
    if net == KeyNet::TooMany {
        anyhow::bail!("too many networks (limit {MAX_NETS})");
    }
    Ok(net)
}

/// The network key of the settings file, read without touching anything.
pub(crate) fn peek_file_key(path: &Path) -> Option<String> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p.settings.net.network_key,
        _ => None,
    }
}

/// The key of every network slot saved beside the main file (`None` for a slot without a readable key).
pub(crate) fn saved_slot_keys(main: &Path) -> Vec<Option<String>> {
    list_net_slots(main)
        .iter()
        .map(|s| slot_key(&s.path))
        .collect()
}

fn slot_key(path: &Path) -> Option<String> {
    match load_net_file(path) {
        LoadedNetFile::Parsed(p) => p.net.network_key,
        _ => None,
    }
}

/// A slot made ready but not yet written: its id and the run settings it will have, secret included.
pub(crate) struct PreparedSlot {
    pub(crate) id: NetSlotId,
    pub(crate) run: RunConfig,
}

/// Picks the id and builds the settings of a new slot, with its own secret. Nothing is written.
pub(crate) fn prepare_new_slot(main: &RunConfig, now: u64) -> anyhow::Result<PreparedSlot> {
    let main_path = main_file_path(main)?;
    let (id, run) = new_net_slot(main, &main_path, &main.file_settings.net, &|_| false, now)?;
    Ok(PreparedSlot { id, run })
}

fn main_file_path(main: &RunConfig) -> anyhow::Result<std::path::PathBuf> {
    main.file_report
        .path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("the run has no settings file"))
}

/// Prepares a slot and saves it with the key; see `save_prepared_slot`.
pub(crate) fn add_net_slot(
    main: &RunConfig,
    key_text: &str,
    typed_domains: &[String],
    peers: &[PeerAddr],
    now: u64,
) -> anyhow::Result<NetSlotId> {
    let slot = prepare_new_slot(main, now)?;
    save_prepared_slot(main, slot, key_text, typed_domains, peers, now)
}

/// Saves `nets/<id>/config.json` with the key under the slot's own secret and seeds the slot with the invite's
/// peer addresses and the site list (the main network's plus the ones typed with `--domain`). The main file is
/// not written. `Err` when nothing was saved; a failed seed is only a line in the log.
pub(crate) fn save_prepared_slot(
    main: &RunConfig,
    slot: PreparedSlot,
    key_text: &str,
    typed_domains: &[String],
    peers: &[PeerAddr],
    now: u64,
) -> anyhow::Result<NetSlotId> {
    let main_path = main_file_path(main)?;
    let PreparedSlot { id, run: slot } = slot;
    let mut net = slot.file_settings.net.clone();
    net.network_key = Some(key_text.to_string());
    let path = net_slot_path(&main_path, &id)
        .ok_or_else(|| anyhow::anyhow!("the main slot has no slot file"))?;
    save_net_file(&path, None, now, now, &slot.secret, &net)
        .map_err(|e| anyhow::anyhow!("{}", save_text(e)))?;
    let domains = merged(read_main_domains(&main_path), typed_domains);
    seed_slot(&path, key_text, &slot.secret, &domains, peers, now);
    say("network", format!("added a network ({id})"));
    Ok(id)
}

/// Writes the neighbour cache and the site list of a new slot; each failure is a line in the log.
fn seed_slot(
    path: &Path,
    key_text: &str,
    secret: &NodeSecret,
    domains: &[String],
    peers: &[PeerAddr],
    now: u64,
) {
    if let Ok(parsed) = parse_user_input(key_text) {
        let network_id = parsed.key.subkeys().network_id_hex();
        if let Err(why) = seed_peers_cache(path, &network_id, peers, now) {
            say("network", format!("peer addresses not seeded: {why}"));
        }
    }
    if let Err(why) = save_invited_peers(path, peers) {
        say("network", format!("invited peers not saved: {why}"));
    }
    if let Err(why) = seed_domains(path, secret, domains, now * 1_000_000) {
        say("network", format!("site list not seeded: {why}"));
    }
}

fn merged(mut list: Vec<String>, typed: &[String]) -> Vec<String> {
    for d in typed {
        if !list.contains(d) {
            list.push(d.clone());
        }
    }
    list
}

/// The site list of the main network as saved beside the main file. A missing file is an empty list,
/// an unreadable one is an empty list with a line in the log.
pub(crate) fn read_main_domains(main: &Path) -> Vec<String> {
    let Some(dir) = main
        .parent()
        .and_then(|p| replica_file::replica_dir(&p.join("docs-store")))
    else {
        return Vec::new();
    };
    let id = crate::members::sync::replica::ids::domains_id();
    match replica_file::load(&replica_file::path_for(&dir, "domains", &id), &id) {
        Ok(Loaded::Main(_, e)) | Ok(Loaded::Backup(_, e)) => live_domains(&e),
        Ok(Loaded::Fresh) => Vec::new(),
        Err(e) => {
            say(
                "domains",
                format!("the saved site list was not read: {e:?}"),
            );
            Vec::new()
        }
    }
}

/// The domains whose newest record (per author and key) is not a tombstone, in the order of first appearance.
fn live_domains(entries: &[WireEntry]) -> Vec<String> {
    let mut newest: Vec<&WireEntry> = Vec::new();
    for e in entries
        .iter()
        .filter(|e| e.key.starts_with(LIST_PREFIX.as_bytes()))
    {
        match newest
            .iter()
            .position(|o| o.author == e.author && o.key == e.key)
        {
            Some(i) if newest[i].ts_micros < e.ts_micros => newest[i] = e,
            Some(_) => {}
            None => newest.push(e),
        }
    }
    let live = newest.iter().filter(|e| !e.value.is_empty());
    merged(
        Vec::new(),
        &live
            .filter_map(|e| domain_from_key(&e.key))
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
#[path = "run_invite_addnet_tests.rs"]
mod tests;
