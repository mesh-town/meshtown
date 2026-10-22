//! The list of networks for `/api/state` and the selection of addresses open to the network's devices.

use super::access::IfaceAddr;
use super::iface_gate::{judge_iface, By, IfaceView};
use super::iface_kind::{kind_of, IfaceKind};
use crate::config_file::{IfaceFilter, ListenAccess};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct NetEntry {
    pub name: String,
    pub addrs: Vec<String>,
    pub default_route: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<IfaceKind>,
    pub present: bool,
    pub open: bool,
    pub by: By,
}

fn entry(
    mode: ListenAccess,
    filter: &IfaceFilter,
    view: &IfaceView,
    name: &str,
) -> NetEntry {
    let (open, by, _) = judge_iface(mode, filter, view, Some(name)).meta();
    let addrs = view.addrs.iter().filter(|a| a.name == name);
    NetEntry {
        name: name.to_string(),
        addrs: addrs.map(|a| a.ip.to_string()).collect(),
        default_route: view.default_iface() == Some(name),
        kind: kind_of(name),
        present: true,
        open,
        by,
    }
}

fn absent(name: &str) -> NetEntry {
    NetEntry {
        name: name.to_string(),
        addrs: Vec::new(),
        default_route: false,
        kind: kind_of(name),
        present: false,
        open: false,
        by: By::List,
    }
}

fn view_names(view: &IfaceView) -> Vec<&str> {
    let default = view.default_iface();
    let mut names: Vec<&str> = view.addrs.iter().map(|a| a.name.as_str()).collect();
    names.sort_by_key(|n| (Some(*n) != default, *n));
    names.dedup();
    names
}

/// The machine's networks: default route first, then by name; then the names from the list that are absent.
pub(crate) fn networks(mode: ListenAccess, filter: &IfaceFilter, view: &IfaceView) -> Vec<NetEntry> {
    let names = view_names(view);
    let mut out: Vec<NetEntry> = names.iter().map(|n| entry(mode, filter, view, n)).collect();
    if let IfaceFilter::Only(listed) = filter {
        let missing = listed.iter().filter(|n| !names.contains(&n.as_str()));
        out.extend(missing.map(|n| absent(n)));
    }
    out
}

/// From `lan_addrs()` — only the addresses of admitted interfaces; order is kept.
pub(crate) fn admitted_lan(
    mode: ListenAccess,
    filter: &IfaceFilter,
    view: &IfaceView,
    lan: Vec<IfaceAddr>,
) -> Vec<IfaceAddr> {
    let admitted = |name: &str| judge_iface(mode, filter, view, Some(name)).meta().0;
    lan.into_iter().filter(|(name, _)| admitted(name)).collect()
}

#[cfg(test)]
#[path = "iface_nets_tests.rs"]
mod tests;
