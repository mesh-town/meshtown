//! The summary of all the node's networks (`GET /api/overview`): one `SlotView` per network, parsed softly —
//! a field of the wrong type reads as zero, `None` or `false`, never as an error.

use serde_json::Value;

/// One network of the summary together with the state of its slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotView {
    pub slot: String,
    pub label: Option<String>,
    pub joining: bool,
    /// The slot holds a network key (`id_hex` is a string).
    pub keyed: bool,
    /// The network key as the summary gives it; `None` when the slot holds none.
    pub id_hex: Option<String>,
    /// The network that carries the browser proxy.
    pub proxy: bool,
    pub status: String,
    pub peers_online: u32,
    pub peers_total: u32,
    pub members_voting: u32,
    pub members_admitting: u32,
    /// The network is administered (`kind` is `Managed`); a peer network is the default.
    pub managed: bool,
    /// We are an admin of the network.
    pub admin: bool,
    /// We may issue invites here; an unreadable or missing flag never forbids it.
    pub can_invite: bool,
    /// The `/api/state` document of the slot; `Null` when the slot has none.
    pub state: Value,
}

/// The networks of a summary in the order of `nets`; `None` when the document has no `nets` and `slots` arrays
/// (a reply of the single-network `/api/state` form).
pub fn parse_overview(v: &Value) -> Option<Vec<SlotView>> {
    let nets = v.get("nets")?.as_array()?;
    let slots = v.get("slots")?.as_array()?;
    Some(nets.iter().map(|net| slot_view(net, slots)).collect())
}

/// The networks the menu shows: with a key, or being joined right now.
pub fn visible_slots(all: Vec<SlotView>) -> Vec<SlotView> {
    all.into_iter().filter(|s| s.keyed || s.joining).collect()
}

/// The network of the menu's single view: the one with the proxy, otherwise the first.
pub fn main_slot(vis: &[SlotView]) -> Option<&SlotView> {
    vis.iter().find(|s| s.proxy).or_else(|| vis.first())
}

/// A note of the summary: a network that an admin deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub id: u64,
    pub label: String,
}

/// The deleted-network notes of a summary in array order; entries without a numeric `id` or of another kind are skipped.
pub fn parse_notices(v: &Value) -> Vec<Notice> {
    let list = v.get("notices").and_then(Value::as_array);
    list.map(|l| l.iter().filter_map(notice).collect())
        .unwrap_or_default()
}

fn notice(n: &Value) -> Option<Notice> {
    (n.get("kind")?.as_str()? == "net_deleted").then_some(())?;
    Some(Notice {
        id: n.get("id")?.as_u64()?,
        label: text(n, "label").unwrap_or_default(),
    })
}

fn slot_view(net: &Value, slots: &[Value]) -> SlotView {
    let slot = text(net, "id").unwrap_or_default();
    let state = state_of(&slot, slots);
    let own = state.get("network").unwrap_or(&Value::Null);
    SlotView {
        managed: kind_of(net).or(kind_of(own)) == Some("Managed"),
        admin: opt_flag(net, "me_admin")
            .unwrap_or_else(|| own.get("me_role").and_then(Value::as_str) == Some("admin")),
        can_invite: opt_flag(net, "can_invite")
            .or_else(|| opt_flag(own, "can_invite"))
            .unwrap_or(true),
        label: text(net, "label"),
        joining: flag(net, "joining"),
        id_hex: text(net, "id_hex"),
        keyed: net.get("id_hex").is_some_and(Value::is_string),
        proxy: flag(net, "proxy"),
        status: text(net, "status").unwrap_or_default(),
        peers_online: count(net, "peers_online"),
        peers_total: count(net, "peers_total"),
        members_voting: count(net, "members_voting"),
        members_admitting: count(net, "members_admitting"),
        state,
        slot,
    }
}

fn state_of(slot: &str, slots: &[Value]) -> Value {
    let found = slots
        .iter()
        .find(|s| s.get("slot").and_then(Value::as_str) == Some(slot));
    found
        .and_then(|s| s.get("state"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn kind_of(v: &Value) -> Option<&str> {
    v.get("kind").and_then(Value::as_str)
}

fn opt_flag(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn count(v: &Value, key: &str) -> u32 {
    let n = v.get(key).and_then(Value::as_u64).unwrap_or(0);
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
