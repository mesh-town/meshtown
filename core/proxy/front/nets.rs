//! The directory of published proxy configurations of the running networks, shared by the listener and the seat.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::config::config_file::NetSlotId;
use crate::proxy::ProxyConfig;

/// Published configuration of every running network; clones share the same map.
#[derive(Clone, Default)]
pub(crate) struct NetDirectory(Arc<RwLock<BTreeMap<NetSlotId, ProxyConfig>>>);

impl NetDirectory {
    pub(crate) fn put(&self, id: &NetSlotId, cfg: ProxyConfig) {
        self.0
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), cfg);
    }

    pub(crate) fn remove(&self, id: &NetSlotId) {
        self.0.write().unwrap_or_else(|e| e.into_inner()).remove(id);
    }

    pub(crate) fn get(&self, id: &NetSlotId) -> Option<ProxyConfig> {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }
}

#[cfg(test)]
#[path = "nets_tests.rs"]
mod tests;
