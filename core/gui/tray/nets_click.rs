//! Clicks on the menu of several networks: the id of an item → what it targets → the page link or the node commands.
//! Pure functions: `Core` owns the state and sends the results.

use super::node_cmd::NodeCmd;
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::members_note::with_anchor;

/// What a menu item stands for. A `slot` of `None` is the network of the flat menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    Open,
    Admit,
    /// "Add a network…": opens the add form of the page.
    Add,
    /// A deleted-network note, by its number: opens the member list.
    Notice(u64),
    Node,
    Quit,
    Invite(Option<String>),
    Serve(Option<String>),
    Exit {
        slot: Option<String>,
        id: String,
    },
    Directly,
    Members(String),
    Unknown,
}

/// What a click needs to know about one shown network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetFact {
    pub slot: String,
    pub label: String,
    pub serves_exit: bool,
    pub exit_chosen: bool,
}

/// The facts about the shown networks; `nets` is empty unless two or more networks are shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuFacts {
    pub nets: Vec<NetFact>,
    /// The network a click on the "request" item opens the members page of.
    pub admit_slot: Option<String>,
}

/// An item id → its target. An unknown or empty id is `Unknown`.
pub fn parse_menu_id(id: &str) -> MenuTarget {
    match id.split_once(':') {
        None => plain_target(id),
        Some((kind, rest)) => prefixed_target(kind, rest),
    }
}

fn plain_target(id: &str) -> MenuTarget {
    match id {
        "open" => MenuTarget::Open,
        "admit" => MenuTarget::Admit,
        "add" => MenuTarget::Add,
        "node" => MenuTarget::Node,
        "quit" => MenuTarget::Quit,
        "invite" => MenuTarget::Invite(None),
        "serve" => MenuTarget::Serve(None),
        _ => MenuTarget::Unknown,
    }
}

fn prefixed_target(kind: &str, rest: &str) -> MenuTarget {
    match (kind, rest) {
        (_, "") if kind != "exit" => MenuTarget::Unknown,
        ("exit", "") => MenuTarget::Directly,
        ("exit", rest) => exit_target(rest),
        ("invite", slot) => MenuTarget::Invite(Some(slot.to_string())),
        ("serve", slot) => MenuTarget::Serve(Some(slot.to_string())),
        ("peer" | "members", slot) => MenuTarget::Members(slot.to_string()),
        ("notice", id) => id.parse().map_or(MenuTarget::Unknown, MenuTarget::Notice),
        _ => MenuTarget::Unknown,
    }
}

fn exit_target(rest: &str) -> MenuTarget {
    match rest.split_once(':') {
        None => MenuTarget::Exit {
            slot: None,
            id: rest.to_string(),
        },
        Some((slot, id)) if !slot.is_empty() && !id.is_empty() => MenuTarget::Exit {
            slot: Some(slot.to_string()),
            id: id.to_string(),
        },
        Some(_) => MenuTarget::Unknown,
    }
}

/// The facts of a menu: its networks (only for two or more), which of them have a chosen exit, and the network with a request.
pub fn menu_facts(menu: &ExitMenu) -> MenuFacts {
    let nets = menu
        .nets
        .iter()
        .map(|net| NetFact {
            slot: net.slot.clone(),
            label: net.label.clone(),
            serves_exit: net.serves_exit,
            exit_chosen: exit_chosen(menu, &net.slot),
        })
        .collect();
    let admitting = menu.nets.iter().find(|n| n.admitting > 0);
    MenuFacts {
        nets,
        admit_slot: admitting.map(|n| n.slot.clone()),
    }
}

fn exit_chosen(menu: &ExitMenu, slot: &str) -> bool {
    let group = menu.groups.iter().find(|g| g.slot == slot);
    group.is_some_and(|g| g.items.iter().any(|item| item.checked))
}

/// The page address for a click that opens the page; `None` — the click does not open it.
pub fn link_for(target: &MenuTarget, base: &str, facts: &MenuFacts) -> Option<String> {
    match target {
        MenuTarget::Open => Some(base.to_string()),
        MenuTarget::Members(slot) => Some(with_anchor(base, &format!("peers/{slot}"))),
        MenuTarget::Admit => Some(with_anchor(base, &admit_anchor(facts))),
        MenuTarget::Add => Some(with_anchor(base, "peers/add")),
        MenuTarget::Notice(_) => Some(with_anchor(base, "peers")),
        _ => None,
    }
}

fn admit_anchor(facts: &MenuFacts) -> String {
    match (&facts.admit_slot, facts.nets.len() >= 2) {
        (Some(slot), true) => format!("peers/{slot}"),
        _ => "peers".to_string(),
    }
}

/// The node commands a click sends; empty for the clicks `Core` handles itself.
pub fn node_cmd_for(
    target: &MenuTarget,
    flat_slot: Option<&str>,
    serves_exit: bool,
    facts: &MenuFacts,
) -> Vec<NodeCmd> {
    let flat = flat_slot.map(str::to_string);
    match target {
        MenuTarget::Invite(None) => vec![NodeCmd::CopyInvite(flat)],
        MenuTarget::Invite(slot) => vec![NodeCmd::CopyInvite(slot.clone())],
        MenuTarget::Serve(None) => vec![serve_cmd(flat, serves_exit)],
        MenuTarget::Serve(Some(slot)) => {
            vec![serve_cmd(Some(slot.clone()), net_serves(facts, slot))]
        }
        MenuTarget::Exit { slot, id } => vec![NodeCmd::ToggleExit {
            slot: slot.clone().or(flat),
            id: id.clone(),
        }],
        MenuTarget::Directly => clear_cmds(flat, facts),
        _ => Vec::new(),
    }
}

fn serve_cmd(slot: Option<String>, serves_now: bool) -> NodeCmd {
    NodeCmd::SetExitServer {
        slot,
        on: !serves_now,
    }
}

fn net_serves(facts: &MenuFacts, slot: &str) -> bool {
    facts
        .nets
        .iter()
        .any(|net| net.slot == slot && net.serves_exit)
}

/// "Directly" clears the chosen exit of the flat network, or of every network that has one.
fn clear_cmds(flat: Option<String>, facts: &MenuFacts) -> Vec<NodeCmd> {
    if facts.nets.len() < 2 {
        return vec![NodeCmd::ClearExits(flat)];
    }
    let chosen = facts.nets.iter().filter(|net| net.exit_chosen);
    chosen
        .map(|net| NodeCmd::ClearExits(Some(net.slot.clone())))
        .collect()
}

#[cfg(test)]
#[path = "nets_click_tests.rs"]
mod tests;
