//! Reading and writing publications in the shared document: `publish`/`unpublish`/`own_ports` for the announcer,
//! `read_all` for the page. The read deadline is set by the caller.

use crate::transport::net::{live, NetError, PeerId, Replica, ReplicaEntry};
use anyhow::Result;
use std::sync::Arc;

use super::record::{
    decode_port_record, encode_port_record, port_from_key, port_key, PortRecord, PORT_PREFIX,
};
use super::types::Published;

/// The publications document over a replica (our own author is `r.me()`).
#[derive(Clone)]
pub struct PortsDoc {
    r: Arc<dyn Replica>,
}

/// A publication of one node; `record: None` — the value did not arrive, is corrupt or has a foreign version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortView {
    pub author: PeerId,
    pub port: u16,
    pub record: Option<PortRecord>,
}

impl PortsDoc {
    pub fn new(r: Arc<dyn Replica>) -> PortsDoc {
        PortsDoc { r }
    }

    /// Writes a publication entry under `port/tcp/<port>/`.
    pub async fn publish(&self, p: &Published) -> Result<()> {
        let value = encode_port_record(&PortRecord::from(p));
        self.r
            .set_own(&port_key(p.port), value.into())
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(())
    }

    /// Removes a publication by the exact key with a slash; if there was no port, a harmless tombstone.
    pub async fn unpublish(&self, port: u16) -> Result<()> {
        self.r
            .del_own(&port_key(port))
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(())
    }

    /// Ports of our own live entries (not tombstones), ascending; the value is not read.
    pub async fn own_ports(&self) -> Result<Vec<u16>> {
        let entries = self.scan_ports().await?;
        let me = self.r.me();
        let mut ports = Vec::new();
        for e in entries.iter().filter(|e| e.author == me) {
            ports.extend(port_from_key(&e.key));
        }
        ports.sort_unstable();
        ports.dedup();
        Ok(ports)
    }

    /// Publications of all nodes (ours included) by author bytes, then port.
    pub async fn read_all(&self) -> Result<Vec<PortView>> {
        let entries = self.scan_ports().await?;
        let mut out: Vec<PortView> = entries.iter().filter_map(view_of).collect();
        out.sort_by(|a, b| (a.author.as_bytes(), a.port).cmp(&(b.author.as_bytes(), b.port)));
        Ok(out)
    }

    /// Live publication entries of all authors.
    async fn scan_ports(&self) -> Result<Vec<ReplicaEntry>> {
        let entries = self
            .r
            .scan(PORT_PREFIX.as_bytes())
            .await
            .map_err(NetError::into_anyhow)?;
        Ok(live(entries))
    }
}

fn view_of(entry: &ReplicaEntry) -> Option<PortView> {
    let port = port_from_key(&entry.key)?;
    Some(PortView {
        author: entry.author,
        port,
        record: decode_port_record(&entry.value),
    })
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
