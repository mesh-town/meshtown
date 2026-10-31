//! The `config` command group: the settings file and the settings of a running node.
//!
//! - `cmd` - the command line types.
//! - `path` - `config path`.
//! - `get`, `set` - `config get` and `config set`, on the node or on the file.
//! - `file` - the settings file as the two commands read and write it.
//! - `assign` - the `path=value` text of `--set`.

pub(super) mod assign;
mod cmd;
mod file;
mod get;
mod path;
mod set;

pub(crate) use cmd::{ConfigArgs, ConfigCmd};

use crate::cli::args::Globals;

pub(crate) async fn run(args: ConfigArgs, g: &Globals) -> anyhow::Result<()> {
    match args.cmd {
        ConfigCmd::Path => path::run(g),
        ConfigCmd::Get { path } => get::run(path.as_deref(), g),
        ConfigCmd::Set { path, value } => set::run(&path, &value, g),
    }
}
