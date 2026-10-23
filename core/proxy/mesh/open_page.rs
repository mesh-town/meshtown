//! Pages for mesh hosts that cannot be opened: collecting the fields and serving the page as an upstream.

use std::time::Duration;

use tokio::time::timeout;

use crate::node::node_name::read_node_names;
use crate::ports_sharing::{PortRecord, PortsDoc};
use crate::transport::net::PeerId;
use crate::proxy::front::NetEntry;
use crate::proxy::path::OpenedUpstream;

use super::catalog::{addresses, suggest_labels, Device};
use super::host::{mesh_url, MeshHost, MESH_SUFFIX};
use super::page::{MeshChoice, MeshPage, OpenPort, PageKind};
use super::page_serve::serve_page;

/// How long the shared documents may take to be read for a page.
const DOC_READ_TIMEOUT: Duration = Duration::from_secs(2);
/// The name shown when the node's own name is not in the document yet.
const UNNAMED_SELF: &str = "this node";

/// What was asked, and the device it resolved to (if any).
pub(super) struct Ask<'a> {
    host: &'a MeshHost,
    port: u16,
    device: Option<&'a Device>,
    address: String,
    pub(super) entry: &'a NetEntry,
}

/// The ports a device has opened to this node, without the requested one.
struct Listing {
    open: Vec<OpenPort>,
    /// The requested port is published by the device (open to us or not).
    published: bool,
}

impl<'a> Ask<'a> {
    /// A request that matched no single device. `entries` is never empty when called from `open_target`.
    pub(super) fn bare(entries: &'a [NetEntry], host: &'a MeshHost, port: u16) -> Ask<'a> {
        Ask { host, port, device: None, address: String::new(), entry: &entries[0] }
    }

    pub(super) fn of_device(
        entries: &'a [NetEntry],
        list: &[Device],
        device: &'a Device,
        host: &'a MeshHost,
        port: u16,
    ) -> Ask<'a> {
        let entry = entries.iter().find(|e| e.id == device.net).unwrap_or(&entries[0]);
        let address = addresses(list).remove(&device.peer).unwrap_or_default();
        Ask { host, port, device: Some(device), address, entry }
    }

    /// Serves `kind` as the upstream; `label` goes to the journal line.
    pub(super) async fn page(&self, label: &str, kind: PageKind) -> OpenedUpstream {
        let page = MeshPage { me: self.me_name().await, host: self.host_text(), kind };
        let (send, recv) = serve_page(page);
        OpenedUpstream::stream(format!("mesh page: {label}"), send, recv)
    }

    /// The device is not in the network: `Offline` if it publishes the port, else `NotOpened` marked offline.
    pub(super) async fn offline(&self) -> OpenedUpstream {
        let listing = self.listing().await;
        if !listing.published {
            return self.page("not-opened", self.not_opened_kind(listing.open, true)).await;
        }
        let kind = PageKind::Offline {
            device: self.device_name(),
            url: self.url(),
            port: self.port,
            open: listing.open,
        };
        self.page("offline", kind).await
    }

    pub(super) async fn not_opened(&self) -> OpenedUpstream {
        let listing = self.listing().await;
        self.page("not-opened", self.not_opened_kind(listing.open, false)).await
    }

    pub(super) async fn no_service(&self, timed_out: bool) -> OpenedUpstream {
        let kind = PageKind::NoService {
            device: self.device_name(),
            url: self.url(),
            port: self.port,
            timed_out,
        };
        self.page("no-service", kind).await
    }

    fn not_opened_kind(&self, open: Vec<OpenPort>, offline: bool) -> PageKind {
        PageKind::NotOpened {
            device: self.device_name(),
            url: self.url(),
            port: self.port,
            open,
            offline,
        }
    }

    fn device_name(&self) -> String {
        self.device.map(|d| d.name.clone()).unwrap_or_default()
    }

    fn url(&self) -> String {
        mesh_url(&self.address, self.port)
    }

    fn host_text(&self) -> String {
        if self.host.label.is_empty() {
            return MESH_SUFFIX.to_string();
        }
        format!("{}.{MESH_SUFFIX}", self.host.label)
    }

    /// The node's own name from the shared document of the network in `self.entry` (the device's network, else the lowest slot).
    async fn me_name(&self) -> String {
        let me = self.entry.cfg.route.me();
        let replica = self.entry.cfg.route.shared_replica();
        let names = timeout(DOC_READ_TIMEOUT, read_node_names(&*replica)).await;
        let found = names.ok().and_then(Result::ok).and_then(|mut names| names.remove(&me));
        found.unwrap_or_else(|| UNNAMED_SELF.to_string())
    }

    /// Reads the ports document; any failure reads as an empty list.
    async fn listing(&self) -> Listing {
        let Some(device) = self.device else {
            return Listing { open: Vec::new(), published: false };
        };
        let doc = PortsDoc::new(self.entry.cfg.route.shared_replica());
        let read = timeout(DOC_READ_TIMEOUT, doc.read_all()).await;
        let views = read.ok().and_then(Result::ok).unwrap_or_default();
        let me = self.entry.cfg.route.me();
        let mine = views.into_iter().filter(|v| v.author == device.peer);
        let mut listing = Listing { open: Vec::new(), published: false };
        for view in mine {
            let Some(record) = view.record else { continue };
            listing.published |= view.port == self.port && record.name_for(&me).is_some();
            listing.open.extend(self.open_port(view.port, &record, &me));
        }
        listing
    }

    /// A published port other than the requested one that is open to `me`.
    fn open_port(&self, port: u16, record: &PortRecord, me: &PeerId) -> Option<OpenPort> {
        if port == self.port {
            return None;
        }
        let name = record.name_for(me)?.to_string();
        Some(OpenPort { port, name, url: mesh_url(&self.address, port) })
    }
}

/// `NotFound`: the label as typed and up to three close labels as links.
pub(super) fn not_found(list: &[Device], host: &MeshHost, port: u16) -> PageKind {
    let suggest = suggest_labels(list, &host.label);
    PageKind::NotFound {
        label: host.label.clone(),
        suggest: suggest.iter().map(|label| mesh_url(label, port)).collect(),
    }
}

/// `Ambiguous`: the candidates ordered by their addresses.
pub(super) fn ambiguous(list: &[Device], found: Vec<Device>, port: u16) -> PageKind {
    let issued = addresses(list);
    let mut choices: Vec<(String, MeshChoice)> = found
        .into_iter()
        .map(|d| {
            let address = issued.get(&d.peer).cloned().unwrap_or_default();
            let url = mesh_url(&address, port);
            (address, MeshChoice { url, name: d.name, online: d.online })
        })
        .collect();
    choices.sort_by(|a, b| a.0.cmp(&b.0));
    PageKind::Ambiguous { choices: choices.into_iter().map(|(_, c)| c).collect() }
}
