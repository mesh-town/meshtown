//! The process part of the settings (`ProcessSettings`): what the process host owns, not the node's network.
//! What isn't here: the network fields (`net_settings.rs`) and JSON parsing (`parse.rs`).

use super::iface_filter::IfaceFilter;
use super::settings::ListenAccess;
use super::slot_store::NetSlotId;
use crate::node::machine_memory::NODE_MEMORY_WARN_MIB_DEFAULT;

/// The fields of the `config` document that the running node does not read: SOCKS, the page, the log,
/// the memory guard, the pass-through `autostart`/`full_tunnel`. Defaults live only in `Default`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessSettings {
    pub socks_enabled: bool,        // socks.enabled
    pub socks_port: u16,            // socks.port
    pub socks_access: ListenAccess, // socks.access   — who may use the proxy
    pub socks_ifaces: IfaceFilter,  // socks.ifaces — over which networks the proxy may be used
    pub proxy_net: NetSlotId,       // proxy.net — which network's exit the local proxy uses
    pub web_enabled: bool,          // web.enabled       — pass-through
    pub web_port: u16,              // web.port          — pass-through
    pub web_access: ListenAccess,   // web.access      — who may open the page
    pub web_ifaces: IfaceFilter,    // web.ifaces — over which networks the page may be opened
    pub log_ring_lines: u64,        // log.ring_lines    — pass-through
    pub autostart_enabled: bool,    // autostart.enabled — pass-through, always false
    pub full_tunnel_enabled: bool,  // full_tunnel.enabled — pass-through, always false
    pub node_memory_warn_mib: u64, // node.memory_warn_mib — memory guard summary threshold, MiB; 0 — off
    pub node_memory_exit_mib: u64, // node.memory_exit_mib — memory guard exit threshold, MiB; 0 — off
    pub exit_max_streams: u64, // exit.max_streams — open exit streams per process; 0 = by machine memory
}

impl Default for ProcessSettings {
    /// Defaults per `docs/CONFIG-FILE.md`, section 3, and `docs/WEB-API-config.md`, section 2.
    fn default() -> Self {
        ProcessSettings {
            socks_enabled: true,
            socks_port: crate::proxy::DEFAULT_SOCKS_PORT,
            socks_access: ListenAccess::Localhost,
            socks_ifaces: IfaceFilter::Auto,
            proxy_net: NetSlotId::main(),
            web_enabled: true,
            web_port: 7777,
            web_access: ListenAccess::Localnet,
            web_ifaces: IfaceFilter::Auto,
            log_ring_lines: 2000,
            autostart_enabled: false,
            full_tunnel_enabled: false,
            node_memory_warn_mib: NODE_MEMORY_WARN_MIB_DEFAULT,
            node_memory_exit_mib: 900,
            exit_max_streams: 0,
        }
    }
}
