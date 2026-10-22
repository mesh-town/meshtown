//! The `FIELDS` table: 27 stored paths and 6 computed ones (mode `ReadOnly`, no field in `Settings`).
//! Row order is the file output order (`render.rs`); computed rows sit in their own section.

use crate::node::net_names;

use super::super::settings::{TransportMode, ListenAccess};
use super::kinds_ifaces::IfaceNames;
use super::kinds_list::{
    NetNames, NetSync, OpenedList, PeerList, PublishedList,
};
use super::kinds_scalar::{
    Addr, Choice, BoolKind, Int, MemoryExit, NetKey, NodeName, OffOnly, OptAddr, Port,
};
use super::kinds_text::TextList;
use super::row::{row, Field, Mode};
use super::value::{Computed, SettingsSlot};

/// A field of a `Settings` block (`process.field` or `net.field`) with a kind: access without capturing, so
/// the expression is extended to `'static`.
macro_rules! slot {
    ($block:ident . $field:ident, $kind:expr) => {
        &SettingsSlot { get: |s| &s.$block.$field, set: |s, v| s.$block.$field = v, kind: $kind }
    };
}

#[rustfmt::skip]
pub(crate) static FIELDS: &[Field] = &[
    row!("node.memory_warn_mib", FileOnly, slot!(process.node_memory_warn_mib, Int { min: 0, max: 1_048_576 })),
    row!("node.memory_exit_mib", FileOnly, &MemoryExit),
    row!("node.name", Live, slot!(net.node_name, NodeName)),
    row!(redacted "network.key", FileOnly, slot!(net.network_key, NetKey)),
    row!("network.label", ReadOnly, &Computed),
    row!("network.masked", ReadOnly, &Computed),
    row!("transport.mode", RestartShown, slot!(net.transport_mode, Choice { parse: TransportMode::parse, name: TransportMode::as_str, expected: "direct_and_relay, relay_only or no_relay" })),
    row!("net.transports", FileOnly, &NetNames { names_in_choice: |n| &mut n.transports, allowed: net_names::transports }),
    row!("net.discovery", FileOnly, &NetNames { names_in_choice: |n| &mut n.discovery, allowed: net_names::discovery }),
    row!("net.sync", FileOnly, &NetSync),
    row!("exit.enabled", Live, slot!(net.exit_enabled, BoolKind)),
    row!("socks.enabled", RestartShown, slot!(process.socks_enabled, BoolKind)),
    row!("socks.port", RestartShown, slot!(process.socks_port, Port { min: 1024, max: 65535 })),
    row!("socks.access", RestartShown, slot!(process.socks_access, Choice { parse: ListenAccess::parse, name: ListenAccess::as_str, expected: "localhost or localnet" })),
    row!("socks.ifaces", RestartShown, slot!(process.socks_ifaces, IfaceNames)),
    row!("checks.enabled", Live, slot!(net.checks_enabled, BoolKind)),
    row!("checks.interval_sec", Live, slot!(net.check_interval_sec, Int { min: 30, max: 86_400 })),
    row!("checks.doh_server", RestartShown, slot!(net.doh_server, Addr)),
    row!("checks.provider_dns", RestartShown, slot!(net.provider_dns, OptAddr)),
    row!("checks.geo_markers", FileOnly, slot!(net.geo_markers, TextList { max_items: 32, max_chars: 64 })),
    row!("probe.interval_sec", Live, slot!(net.probe_interval_sec, Int { min: 5, max: 86_400 })),
    row!("probe.full_cycle", FileOnly, slot!(net.probe_full_cycle, BoolKind)),
    row!("route.exits", Live, slot!(net.route_exits, PeerList)),
    row!("ports.published", Live, slot!(net.ports_published, PublishedList)),
    row!("ports.opened", Live, slot!(net.ports_opened, OpenedList)),
    row!("web.enabled", ReadOnly, slot!(process.web_enabled, BoolKind)),
    row!("web.port", ReadOnly, slot!(process.web_port, Port { min: 1024, max: 65535 })),
    row!("web.access", EditOnly, slot!(process.web_access, Choice { parse: ListenAccess::parse, name: ListenAccess::as_str, expected: "localhost or localnet" })),
    row!("web.ifaces", EditOnly, slot!(process.web_ifaces, IfaceNames)),
    row!("log.ring_lines", Live, slot!(process.log_ring_lines, Int { min: 200, max: 20_000 })),
    row!("autostart.enabled", EditOnly, slot!(process.autostart_enabled, OffOnly)),
    row!("autostart.available", ReadOnly, &Computed),
    row!("autostart.reason", ReadOnly, &Computed),
    row!("full_tunnel.enabled", EditOnly, slot!(process.full_tunnel_enabled, OffOnly)),
    row!("full_tunnel.available", ReadOnly, &Computed),
    row!("full_tunnel.reason", ReadOnly, &Computed),
];

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
