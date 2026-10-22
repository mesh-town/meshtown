//! Interface kind by name and the "this is a router" rule for the automatic mode.

use std::net::Ipv4Addr;

use super::iface_gate::IfaceView;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IfaceKind {
    Tunnel,
    Virtual,
}

const TUNNEL_PREFIXES: &[&str] = &[
    "utun",
    "tun",
    "tap",
    "wg",
    "tailscale",
    "zt",
    "nordlynx",
    "proton",
    "ipsec",
    "gpd",
    "cscotun",
    "nebula",
];

const VIRTUAL_PREFIXES: &[&str] = &[
    "docker", "veth", "virbr", "vnet", "vboxnet", "vmnet", "vnic", "bridge", "lxcbr", "lxdbr",
    "incusbr", "podman", "cni", "flannel", "cali", "kube",
];

/// Docker networks: `br-` and exactly 12 hexadecimal digits.
fn is_docker_bridge(lower: &str) -> bool {
    let Some(rest) = lower.strip_prefix("br-") else {
        return false;
    };
    rest.len() == 12 && rest.chars().all(|c| c.is_ascii_hexdigit())
}

fn has_prefix(lower: &str, list: &[&str]) -> bool {
    list.iter().any(|p| lower.starts_with(p))
}

/// Kind by name (case-insensitive); ordinary networks get `None`.
pub(crate) fn kind_of(name: &str) -> Option<IfaceKind> {
    let lower = name.to_lowercase();
    if has_prefix(&lower, TUNNEL_PREFIXES) {
        return Some(IfaceKind::Tunnel);
    }
    let virt = has_prefix(&lower, VIRTUAL_PREFIXES) || is_docker_bridge(&lower);
    virt.then_some(IfaceKind::Virtual)
}

pub(crate) fn is_virtual(name: &str) -> bool {
    kind_of(name).is_some()
}

fn private_non_virtual(a: &super::iface_gate::ViewAddr) -> bool {
    a.broadcast && a.ip.is_private() && !is_virtual(&a.name)
}

fn same_subnet(a: Ipv4Addr, pa: u8, b: Ipv4Addr, pb: u8) -> bool {
    let shift = 32 - u32::from(pa.min(pb).min(32));
    let mask = u32::MAX.checked_shl(shift).unwrap_or(0);
    u32::from(a) & mask == u32::from(b) & mask
}

/// Router automatic mode: forwarding is on, the default route goes through a real interface and
/// there is a second private interface in a different subnet.
pub(crate) fn is_router(view: &IfaceView) -> bool {
    let Some(default) = view.default_iface() else {
        return false;
    };
    if !view.forwarding || is_virtual(default) {
        return false;
    }
    let wan: Vec<_> = view.addrs.iter().filter(|a| a.name == default).collect();
    view.addrs.iter().any(|a| {
        let other = a.name != default && private_non_virtual(a);
        other
            && !wan
                .iter()
                .any(|w| same_subnet(w.ip, w.prefix, a.ip, a.prefix))
    })
}

#[cfg(test)]
#[path = "iface_kind_tests.rs"]
mod tests;
