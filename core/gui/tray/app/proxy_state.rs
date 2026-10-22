//! The tray menu item for the browser proxy: the address if enabled and the reason if not.
//!
//! What it does: `ProxyState` — one type instead of `Option<String>`: `On(address)` or `Off(reason)`,
//! the reason is `ProxyOff::DisabledInSettings` (off in the settings file) or `ProxyOff::NoSocksFlag` (the
//! `--no-socks` flag for this run). Builds it from the tray plan (`of_plan`) and from the node snapshot (`of_snapshot`).
//! What it doesn't do: it knows nothing about the menu text (`tray::menu_build`) or HTTP; the node snapshot doesn't
//! carry the reason — the tray flag is added on top through `with_flag_reason_from`.
//! Links: `tray_role::run::proxy_plan_for` supplies the plan, `exit_menu` puts the state into `ExitMenu`,
//! `tray_role::views::views_from_state_json` sets the flag on top of the snapshot.

use crate::gui::tray::app::snapshot::TraySocks;

/// Why the proxy is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyOff {
    /// In the settings file.
    DisabledInSettings,
    /// By the `--no-socks` flag for this run.
    NoSocksFlag,
}

/// The proxy for the menu item: a `host:port` address or the "off" reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyState {
    On(String),
    Off(ProxyOff),
}

impl ProxyState {
    /// The tray plan (`proxy_plan_for`): a port is a loopback address, a refusal is the reason.
    pub fn of_plan(plan: Result<u16, ProxyOff>) -> Self {
        match plan {
            Ok(port) => ProxyState::On(format!("127.0.0.1:{port}")),
            Err(off) => ProxyState::Off(off),
        }
    }

    /// The node snapshot: `socks.listen`, only if `socks.enabled`; otherwise "in the settings".
    pub fn of_snapshot(socks: &TraySocks) -> Self {
        match socks.listen.clone() {
            Some(listen) if socks.enabled => ProxyState::On(listen),
            _ => ProxyState::Off(ProxyOff::DisabledInSettings),
        }
    }

    /// "Off" and the tray's own plan is the `--no-socks` flag: the reason becomes `NoSocksFlag`.
    pub fn with_flag_reason_from(self, tray_proxy_plan: &ProxyState) -> Self {
        match (&self, tray_proxy_plan) {
            (ProxyState::Off(_), ProxyState::Off(ProxyOff::NoSocksFlag)) => {
                ProxyState::Off(ProxyOff::NoSocksFlag)
            }
            _ => self,
        }
    }
}

#[cfg(test)]
#[path = "proxy_state_tests.rs"]
mod tests;
