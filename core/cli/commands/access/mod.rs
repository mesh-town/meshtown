//! The `access` command group: who can reach the node page and the proxy, and from which networks.
//!
//! - `cmd` - the command line types.
//! - `show` - `access` without an action.
//! - `mode` - `access page` and `access proxy`.
//! - `ifaces` - `access ifaces`.
//! - `settings_io` - the choice between the node and the settings file, and the file edit.

mod cmd;
mod ifaces;
mod mode;
mod settings_io;
mod show;

pub(crate) use cmd::{parse_access, AccessArgs, AccessCmd};

use crate::cli::args::Globals;

pub(crate) async fn run(args: AccessArgs, g: &Globals) -> anyhow::Result<()> {
    match args.cmd {
        None => show::run(g),
        Some(AccessCmd::Page { mode }) => mode::page(mode, g),
        Some(AccessCmd::Proxy { mode }) => mode::proxy(mode, g),
        Some(AccessCmd::Ifaces { names, if_unset }) => ifaces::run(names, if_unset, g),
    }
}
