//! The node's own domain list (its own replica, never reconciled) and the shared network replica.
//!
//! What it does: holds the shared network replica (`REPLICA_NET`) and its own (`REPLICA_DOMAINS`, `own.rs`); adds, removes and reads domains only
//! in its own; on first start copies the former list from the shared replica into it; starts reconciliation
//! of the shared replica with a member, prints its events and parses the console commands
//! `domains add|remove|list`. The shared document carries `chk/`, `node/exit/`, `node/country/`, `node/version/`.
//! What it doesn't do: doesn't check domain reachability (the `chk/` records are the next stage),
//! doesn't limit the number of records; it is not the module that stores to disk — the sync opens
//! the store, and without a directory it lives in memory.
//! Related: `node.rs` opens the list at start and calls `sync_with` on each new member;
//! `output` is the output; keys come from `network_key::SubKeys`.

use std::sync::Arc;

use crate::net::{NetError, PeerId, Replica, ReplicaEvent, ReplicaId};
use anyhow::{bail, Result};
use bytes::Bytes;
use tokio::sync::broadcast::error::RecvError;

mod own;

use crate::output::say;

/// Key prefix of the domain list in the document.
pub const LIST_PREFIX: &str = "list/";
/// Value of a domain record: non-empty, since an empty record in iroh-docs is a tombstone.
pub const ENTRY_VALUE: &[u8] = b"v1";
/// Maximum length of a domain.
pub const MAX_DOMAIN_LEN: usize = 253;
/// Maximum length of a single domain label.
pub const MAX_LABEL_LEN: usize = 63;

/// The host from input: a bare domain or a full page address (scheme, login, port, path,
/// `?`, `#` are dropped; non-ASCII labels are converted to punycode). Empty input is returned as is,
/// so that the earlier checks give their own error.
fn host_from_input(input: &str) -> Result<String> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    let full = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
    let url = url::Url::parse(&full).map_err(|_| {
        anyhow::anyhow!("couldn't parse \"{s}\" as a domain; you can paste the full page address — the domain will be taken from it")
    })?;
    match url.host_str() {
        Some(h) => Ok(h.to_string()),
        None => bail!("no domain in \"{s}\""),
    }
}

/// Normalizes a domain or explains why it doesn't fit.
pub fn normalize_domain(input: &str) -> Result<String> {
    let d = host_from_input(input)?;
    let d = d.trim_end_matches('.').to_ascii_lowercase();
    if d.is_empty() {
        bail!("empty domain");
    }
    if d.len() > MAX_DOMAIN_LEN {
        bail!("domain is longer than {MAX_DOMAIN_LEN} characters");
    }
    for label in d.split('.') {
        if label.is_empty() {
            bail!("empty label in domain \"{d}\"");
        }
        if label.len() > MAX_LABEL_LEN {
            bail!("label is longer than {MAX_LABEL_LEN} characters in domain \"{d}\"");
        }
        if label.starts_with('-') || label.ends_with('-') {
            bail!("label starts or ends with a hyphen in domain \"{d}\"");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            bail!("invalid character in domain \"{d}\": only a-z, 0-9, hyphen and dot are allowed");
        }
    }
    if !d.contains('.') {
        bail!("no dot in domain \"{d}\"");
    }
    Ok(d)
}

/// Key of a domain record: `list/<domain>/`. The trailing `/` is needed because of the prefix
/// semantics of iroh-docs: without it the record `a.com` would overwrite `a.com.org` by the same author.
pub fn entry_key(domain: &str) -> Vec<u8> {
    format!("{LIST_PREFIX}{domain}/").into_bytes()
}

/// The domain from a record key; `None` if the key isn't from the list or the domain isn't normalized.
pub fn domain_from_key(key: &[u8]) -> Option<String> {
    let rest = key.strip_prefix(LIST_PREFIX.as_bytes())?;
    let name = rest.strip_suffix(b"/")?;
    let name = std::str::from_utf8(name).ok()?;
    match normalize_domain(name) {
        Ok(d) if d == name => Some(d),
        _ => None,
    }
}

/// A console command of the running node.
#[derive(Debug, PartialEq, Eq)]
pub enum DomainConsoleCommand {
    Add(String),
    Remove(String),
    List,
}

/// Parses a console line; an empty line is `None`.
pub fn parse_domain_console_line(line: &str) -> Result<Option<DomainConsoleCommand>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(None),
        ["domains", "add", d] => Ok(Some(DomainConsoleCommand::Add((*d).to_string()))),
        ["domains", "remove", d] => Ok(Some(DomainConsoleCommand::Remove((*d).to_string()))),
        ["domains", "list"] => Ok(Some(DomainConsoleCommand::List)),
        _ => bail!(
            "didn't understand the command \"{}\"; available: domains add <domain>, domains remove <domain>, domains list",
            line.trim()
        ),
    }
}

/// The node's domains: the shared network replica and its own list replica (never reconciled).
#[derive(Clone)]
pub struct DomainList {
    shared: Arc<dyn Replica>,
    own: Arc<dyn Replica>,
}

impl DomainList {
    /// Moves the former list from the shared replica into its own (before reconciliation), then enables reconciliation of the shared one.
    pub async fn open(shared: Arc<dyn Replica>, own: Arc<dyn Replica>) -> Result<DomainList> {
        own::migrate_shared_list(&*shared, &*own).await?;
        shared.start_sync().await.map_err(NetError::into_anyhow)?;
        Ok(DomainList { shared, own })
    }

    /// Returns the shared replica id.
    pub fn shared_replica_id(&self) -> ReplicaId {
        self.shared.id()
    }

    /// Returns the id of the own list replica.
    pub fn own_replica_id(&self) -> ReplicaId {
        self.own.id()
    }

    /// The shared network replica: `chk/`, `node/exit/`, `node/country/`, `node/version/`; the domain list is not in it.
    pub fn shared_replica(&self) -> Arc<dyn Replica> {
        self.shared.clone()
    }

    /// The own list replica (never reconciled): domains, local blocks of the membership list.
    pub fn own_replica(&self) -> Arc<dyn Replica> {
        self.own.clone()
    }

    /// The id of this node (the author of the records).
    pub fn me(&self) -> PeerId {
        self.shared.me()
    }

    /// Adds a domain; returns its normalized form.
    pub async fn add(&self, input: &str) -> Result<String> {
        let d = normalize_domain(input)?;
        let value = Bytes::from_static(ENTRY_VALUE);
        self.own
            .set_own(&entry_key(&d), value)
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(d)
    }

    /// Removes a domain: puts a tombstone from its own author on the exact domain key.
    pub async fn remove(&self, input: &str) -> Result<String> {
        let d = normalize_domain(input)?;
        self.own
            .del_own(&entry_key(&d))
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(d)
    }

    /// The current list of the node's domains, alphabetically.
    pub async fn list(&self) -> Result<Vec<String>> {
        own::read_domain_list(&*self.own).await
    }

    /// The number of domains in the list.
    pub async fn count(&self) -> Result<usize> {
        Ok(self.list().await?.len())
    }

    /// Starts a one-off reconciliation of the shared replica with a member.
    pub async fn sync_with(&self, peer: PeerId) -> Result<()> {
        self.shared
            .sync_with(peer)
            .await
            .map_err(NetError::into_anyhow)
    }
}

/// Prints the events of the shared replica: reconciliation results, swarm neighbors (the domain list is not in it);
/// counts insertions in `changes`.
pub async fn log_events(list: DomainList, changes: Arc<crate::node::DocChanges>) {
    let mut events = list.shared_replica().subscribe();
    loop {
        match events.recv().await {
            Ok(event) => log_event(event, &changes),
            Err(RecvError::Lagged(_)) => changes.bump_remote(),
            Err(RecvError::Closed) => return,
        }
    }
}

/// One replica event: a counter or a log line.
fn log_event(event: ReplicaEvent, changes: &crate::node::DocChanges) {
    match event {
        ReplicaEvent::LocalChange => changes.bump_local(),
        ReplicaEvent::RemoteChange => changes.bump_remote(),
        ReplicaEvent::Synced {
            peer,
            outcome: Ok(c),
        } => say(
            "docs",
            format!(
                "sync with {}: received {}, sent {}",
                peer.fmt_short(),
                c.received,
                c.sent
            ),
        ),
        ReplicaEvent::Synced {
            peer,
            outcome: Err(e),
        } => say("docs", format!("sync with {}: error {e}", peer.fmt_short())),
        ReplicaEvent::Neighbor { peer, up: true } => say(
            "docs",
            format!("neighbor + {} in the document swarm", peer.fmt_short()),
        ),
        ReplicaEvent::Neighbor { peer, up: false } => say(
            "docs",
            format!("neighbor - {} in the document swarm", peer.fmt_short()),
        ),
    }
}

/// Runs a console command and prints the result.
pub async fn execute_domain_command(list: &DomainList, cmd: DomainConsoleCommand) {
    match cmd {
        DomainConsoleCommand::Add(input) => match list.add(&input).await {
            Ok(d) => say("domains", format!("+ {d} (added locally)")),
            Err(e) => say("domains", format!("not added \"{input}\": {e:#}")),
        },
        DomainConsoleCommand::Remove(input) => match list.remove(&input).await {
            Ok(d) => say("domains", format!("- {d} (removed locally)")),
            Err(e) => say("domains", format!("not removed \"{input}\": {e:#}")),
        },
        DomainConsoleCommand::List => match list.list().await {
            Ok(all) => {
                say("domains", format!("total {}", all.len()));
                for d in all {
                    say("domains", format!("  {d}"));
                }
            }
            Err(e) => say("domains", format!("couldn't read the list: {e:#}")),
        },
    }
}

#[cfg(test)]
#[path = "domains_tests.rs"]
mod tests;
