//! Registry of transports and discovery sources by the names from the `[net]` settings.
//! Builds a `NetStackBuilder` from a `NetChoice`; an unknown name gives `Config`.

use crate::transport::net::discovery::Discovery;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::facade::NetStackBuilder;
use crate::transport::net::id::NodeSecret;
use crate::transport::net::sync_backend::{SyncBackend, SyncCtor, SyncCtx};
use crate::transport::net::transport::Transport;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// Choice from the settings: names in priority order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetChoice {
    pub transports: Vec<String>,
    pub discovery: Vec<String>,
    pub sync: String,
}

impl Default for NetChoice {
    fn default() -> Self {
        let owned_names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        NetChoice {
            transports: owned_names(&["hyper"]),
            discovery: owned_names(&["invite", "cache", "hyperdht"]),
            sync: "mesh".to_string(),
        }
    }
}

pub struct NetCtx {
    pub secret: NodeSecret,
}

pub type TransportCtor = Box<dyn Fn(&NetCtx) -> NetResult<Arc<dyn Transport>> + Send + Sync>;
pub type DiscoveryCtor = Box<dyn Fn(&NetCtx) -> NetResult<Arc<dyn Discovery>> + Send + Sync>;

#[derive(Default)]
pub struct Registry {
    transports: BTreeMap<&'static str, TransportCtor>,
    discovery: BTreeMap<&'static str, DiscoveryCtor>,
    sync: BTreeMap<&'static str, SyncCtor>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A taken name is a code error: `panic!`.
    pub fn add_transport(&mut self, name: &'static str, ctor: TransportCtor) {
        let old = self.transports.insert(name, ctor);
        assert!(old.is_none(), "transport \"{name}\" is already added");
    }

    /// A taken name is a code error: `panic!`.
    pub fn add_discovery(&mut self, name: &'static str, ctor: DiscoveryCtor) {
        let old = self.discovery.insert(name, ctor);
        assert!(
            old.is_none(),
            "discovery source \"{name}\" is already added"
        );
    }

    /// A taken name is a code error: `panic!`.
    pub fn add_sync(&mut self, name: &'static str, ctor: SyncCtor) {
        let old = self.sync.insert(name, ctor);
        assert!(old.is_none(), "sync \"{name}\" is already added");
    }

    /// An unknown name gives `Config` listing the registered ones.
    pub async fn open_sync(
        &self,
        choice: &NetChoice,
        ctx: SyncCtx,
    ) -> NetResult<Arc<dyn SyncBackend>> {
        let name = choice.sync.as_str();
        let Some(ctor) = self.sync.get(name) else {
            let msg = format!(
                "sync \"{name}\" is not registered; available: {}",
                joined_names(&self.sync)
            );
            return Err(NetError::Config(msg));
        };
        ctor(ctx).await
    }

    /// All names are checked first, the constructors are called after; the first error goes as is.
    pub fn builder(&self, choice: &NetChoice, ctx: &NetCtx) -> NetResult<NetStackBuilder> {
        let transport_names = resolve(&self.transports, &choice.transports, "transport")?;
        let discovery_names = resolve(&self.discovery, &choice.discovery, "discovery source")?;
        let mut builder = NetStackBuilder::new(ctx.secret.clone());
        for name in transport_names {
            builder = builder.transport(self.transports[name](ctx)?);
        }
        for name in &discovery_names {
            builder = builder.discovery(self.discovery[name](ctx)?);
        }
        Ok(builder.discovery_priority(discovery_names))
    }
}

/// "a, b" in alphabetical order, or "none".
fn joined_names<V>(m: &BTreeMap<&'static str, V>) -> String {
    let all: Vec<&str> = m.keys().copied().collect();
    if all.is_empty() {
        return "none".to_string();
    }
    all.join(", ")
}

fn resolve<V>(
    m: &BTreeMap<&'static str, V>,
    want: &[String],
    what: &str,
) -> NetResult<Vec<&'static str>> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for name in want {
        let Some((key, _)) = m.get_key_value(name.as_str()) else {
            let msg = format!(
                "{what} \"{name}\" is not registered; available: {}",
                joined_names(m)
            );
            return Err(NetError::Config(msg));
        };
        if !seen.insert(*key) {
            return Err(NetError::Config(format!(
                "{what} \"{name}\" is specified twice"
            )));
        }
        found.push(*key);
    }
    Ok(found)
}

#[cfg(test)]
#[path = "registry_slot_tests.rs"]
mod slot_tests;
#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
