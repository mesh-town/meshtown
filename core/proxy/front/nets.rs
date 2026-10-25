//! The directory of published proxy configurations of the running networks, shared by the listener and the seat.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::config::config_file::NetSlotId;
use crate::exit::PeerConnPool;
use crate::proxy::mesh::catalog::{addresses, devices, rows_of, Device};
use crate::proxy::mesh::host::{host_url, MESH_SUFFIX};
use crate::proxy::ProxyConfig;
use crate::transport::net::{PeerId, PORT};

/// One running network: its slot, its published configuration and its pool of connections to members for the
/// port protocol (one per network; clones share the connections).
#[derive(Clone)]
pub(crate) struct NetEntry {
    pub id: NetSlotId,
    pub cfg: ProxyConfig,
    pub ports: PeerConnPool,
}

impl NetEntry {
    fn new(id: NetSlotId, cfg: ProxyConfig) -> NetEntry {
        NetEntry {
            id,
            cfg,
            ports: PeerConnPool::new(PORT),
        }
    }
}

/// The mesh address of one device in one network, as written in a browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAddress {
    pub net: NetSlotId,
    pub peer: PeerId,
    /// `<address>.meshtown`.
    pub host: String,
}

impl MeshAddress {
    /// The link to `port` of this device: port 80 is left out, port 443 selects `https`.
    pub fn url(&self, port: u16) -> String {
        host_url(&self.host, port)
    }
}

/// Published configuration of every running network; clones share the same map.
#[derive(Clone, Default)]
pub(crate) struct NetDirectory(Arc<RwLock<BTreeMap<NetSlotId, NetEntry>>>);

impl NetDirectory {
    pub(crate) fn put(&self, id: &NetSlotId, cfg: ProxyConfig) {
        self.0
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), NetEntry::new(id.clone(), cfg));
    }

    pub(crate) fn remove(&self, id: &NetSlotId) {
        self.0.write().unwrap_or_else(|e| e.into_inner()).remove(id);
    }

    pub(crate) fn get(&self, id: &NetSlotId) -> Option<ProxyConfig> {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|entry| entry.cfg.clone())
    }

    /// All running networks, ascending by slot id.
    pub(crate) fn entries(&self) -> Vec<NetEntry> {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// The address of every device listed by the running networks.
    pub(crate) fn mesh_addresses(&self) -> Vec<MeshAddress> {
        let devices = devices(rows_of(&self.entries()));
        let issued = addresses(&devices);
        let host_of = |d: &Device| format!("{}.{MESH_SUFFIX}", issued[&d.peer]);
        devices
            .iter()
            .map(|d| MeshAddress {
                net: d.net.clone(),
                peer: d.peer,
                host: host_of(d),
            })
            .collect()
    }

    /// Like `entries`, but an empty directory gives one entry made of `chosen` for the main slot.
    pub(crate) fn entries_or(&self, chosen: &ProxyConfig) -> Vec<NetEntry> {
        let entries = self.entries();
        if entries.is_empty() {
            return vec![NetEntry::new(NetSlotId::main(), chosen.clone())];
        }
        entries
    }
}

#[cfg(test)]
#[path = "nets_tests.rs"]
mod tests;
