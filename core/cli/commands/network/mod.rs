//! The `network` command group: the networks of this computer and their keys.
//!
//! - `cmd` - the command line types.
//! - `list`, `create`, `key`, `inspect`, `delete` - the actions on a running node.
//! - `key_only` - `create --key-only`, which needs no node.
//! - `key_form` - the one conversion of a key into its machine form.

mod cmd;
mod create;
mod delete;
mod inspect;
mod key;
mod key_form;
mod key_only;
mod list;

pub(crate) use cmd::{NetworkArgs, NetworkCmd};

use crate::cli::args::Globals;

pub(crate) async fn run(args: NetworkArgs, g: &Globals) -> anyhow::Result<()> {
    match args.cmd {
        NetworkCmd::List => list::run(g),
        NetworkCmd::Create {
            label,
            raw,
            key_only,
        } => create::run(&label, raw, key_only, g),
        NetworkCmd::Key { raw } => key::run(raw, g),
        NetworkCmd::Inspect { text } => inspect::run(&text, g),
        NetworkCmd::Delete => delete::run(g),
    }
}
