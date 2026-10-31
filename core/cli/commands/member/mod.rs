//! The `member` group: the members of a network and what an administrator does with them.
//!
//! - `cmd` - the command line types.
//! - `resolve` - which member a name or the start of an id points at.
//! - `list` - `member list`: the table and the hints.
//! - `act` - the actions on one member: the request and the words of the result.

mod act;
mod cmd;
mod list;
mod resolve;

pub(crate) use cmd::MemberArgs;
pub(crate) use list::{hints, rows};

use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::{print_json, print_table, Node};

const HEAD: [&str; 6] = ["ID", "STATUS", "ONLINE", "ADMIN", "BLOCKED", "NAME"];

pub(crate) async fn run(args: MemberArgs, g: &Globals) -> anyhow::Result<()> {
    let command = format!("member {}", args.cmd.verb());
    g.allow(&command, &[Global::Net, Global::Json])?;
    act::reason_of(&args.cmd)?;
    let node = Node::connect(g)?;
    let reply = node.call("GET", &node.net_path("members"), None)?;
    match args.cmd.member() {
        None => show(&reply, g),
        Some(wanted) => act::run(&node, &args.cmd, wanted, &reply, g),
    }
}

fn show(reply: &serde_json::Value, g: &Globals) -> anyhow::Result<()> {
    match g.json {
        true => print_json(reply),
        false => print_list(reply),
    }
    Ok(())
}

fn print_list(reply: &serde_json::Value) {
    print_table(&HEAD, &rows(&reply["members"]));
    hints(reply).iter().for_each(|h| eprintln!("{h}"));
}
