//! Library of Mesh Town: transport, membership, the node, the proxy and the node page.
//!
//! What it does: groups the code by capability, one folder per group.
//! What it doesn't do: it holds no logic of its own - this file only declares the groups.
//! Links: the `meshtown` binary (`cli/main.rs`) calls `cli::run::run`.
//!
//! Groups:
//! - `cli` - command line, subcommands, the node console and the process entry.
//! - `config` - the settings file and local storage next to it.
//! - `exit` - the exit protocol `meshtown/exit/1` and byte forwarding.
//! - `invite` - the network invite string: building it and parsing the `--invite` flag.
//! - `members` - the network key, handshake, membership, access and synchronization.
//! - `node` - node assembly from all modules, its control, state and telemetry.
//! - `ports_sharing` - publishing own TCP ports to members and opening others'.
//! - `proxy` - the local SOCKS5/HTTP server, path choice and the domain list.
//! - `site_checks` - checking whether a domain opens, and how.
//! - `transport` - the network layer: interfaces, HyperDHT, the roster record, the peer cache.
//! - `web_gui` - the node page: built-in HTTP server, its API and the UI files.
//! - `gui` - the tray icon (only in builds with the `gui` feature).
//!
//! The core is the modules without foreign network crates (built with `--no-default-features`);
//! the rest is behind the `net-hyper` feature. A new module goes into the core part only if
//! it does not refer to the node layer.

pub mod cli;
pub mod config;
#[cfg(feature = "net-hyper")]
pub mod exit;
#[cfg(feature = "net-hyper")]
pub mod invite;
pub mod members;
pub mod node;
#[cfg(feature = "net-hyper")]
pub mod ports_sharing;
pub mod proxy;
pub mod site_checks;
pub mod transport;
#[cfg(feature = "net-hyper")]
pub mod web_gui;

#[cfg(all(
    feature = "gui",
    feature = "net-hyper",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
pub mod gui;
