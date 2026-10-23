//! What the proxy listener shows to the outside: its address, who may use it and over which interfaces; plus the
//! journal lines about it.

use std::net::SocketAddr;

use crate::config::config_file::{IfaceFilter, ListenAccess};
use crate::proxy::Admission;
use crate::web_gui::auth::iface_nets::admitted_lan;
use crate::web_gui::auth::iface_os::current_view;

/// The published state of the proxy listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyFace {
    /// Always `127.0.0.1:<port>`; `None` when the listener is off.
    pub addr: Option<SocketAddr>,
    pub access: ListenAccess,
    pub ifaces: IfaceFilter,
}

impl ProxyFace {
    /// No listener.
    pub const OFF: ProxyFace = ProxyFace {
        addr: None,
        access: ListenAccess::Localhost,
        ifaces: IfaceFilter::Auto,
    };
}

/// The log line about SOCKS5 being off: the startup flag or a setting.
pub(crate) fn off_line(by_flag: bool) -> String {
    match by_flag {
        true => "off by the --no-socks flag".to_string(),
        false => "off in settings (socks.enabled: false)".to_string(),
    }
}

/// A note for the log line: in local-network mode the proxy has no password.
pub(crate) fn access_note(access: ListenAccess) -> &'static str {
    match access {
        ListenAccess::Localhost => "",
        ListenAccess::Localnet => " (from the local network, no password)",
    }
}

/// A log line with addresses for the network's devices; only in `localnet` mode.
pub(crate) fn lan_line(adm: &Admission, port: u16) -> Option<String> {
    (adm.access == ListenAccess::Localnet).then(|| {
        let all = crate::web_gui::auth::access::lan_addrs();
        let open = admitted_lan(adm.access, &adm.ifaces, &current_view(), all);
        let list: Vec<String> = open
            .into_iter()
            .map(|(iface, ip)| format!("{ip}:{port} ({iface})"))
            .collect();
        match list.is_empty() {
            true => "no local network address found".to_string(),
            false => format!("addresses for network devices: {}", list.join(", ")),
        }
    })
}
