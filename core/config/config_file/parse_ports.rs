//! Pure parsers of the arrays of the `ports` section (`published`, `opened`): the list is either valid as a whole or
//! dropped as a whole (`[]` and one error with the path), like `route.exits`. The field table calls them.

use std::collections::HashSet;

use crate::transport::net::PeerId;
use serde_json::Value;

use crate::ports_sharing::types::{
    Audience, Opened, Proto, Published, MAX_NAME_CHARS, MAX_PORT_LIST_ENTRIES,
};

fn port_of(v: &Value, key: &str) -> Option<u16> {
    let n = v.get(key)?.as_u64()?;
    u16::try_from(n).ok().filter(|p| *p != 0)
}

fn peer_id_of(v: &Value) -> Option<PeerId> {
    v.as_str()?.parse::<PeerId>().ok()
}

fn audience_of(v: &Value) -> Option<Audience> {
    match v.get("for")? {
        Value::String(s) if s == "all" => Some(Audience::All),
        Value::Array(a) if a.len() <= MAX_PORT_LIST_ENTRIES => {
            let ids = a.iter().map(peer_id_of).collect::<Option<Vec<_>>>()?;
            Some(Audience::Nodes(ids))
        }
        _ => None,
    }
}

/// `proto` is optional (TCP when absent); only the strings `"tcp"` and `"udp"` are accepted.
fn proto_of(v: &Value) -> Option<Proto> {
    match v.get("proto") {
        None => Some(Proto::Tcp),
        Some(p) => Proto::parse(p.as_str()?),
    }
}

fn name_of(v: &Value) -> Option<String> {
    match v.get("name") {
        None => Some(String::new()),
        Some(Value::String(s)) if s.chars().count() <= MAX_NAME_CHARS => Some(s.clone()),
        Some(_) => None,
    }
}

fn published_one(v: &Value) -> Option<Published> {
    Some(Published {
        port: port_of(v, "port")?,
        proto: proto_of(v)?,
        name: name_of(v)?,
        audience: audience_of(v)?,
    })
}

pub(super) fn parse_published_list(a: &[Value]) -> Option<Vec<Published>> {
    let list = a.iter().map(published_one).collect::<Option<Vec<_>>>()?;
    let mut seen = HashSet::new();
    let unique = list.iter().all(|p| seen.insert((p.proto, p.port)));
    (list.len() <= MAX_PORT_LIST_ENTRIES && unique).then_some(list)
}

fn opened_one(v: &Value) -> Option<Opened> {
    Some(Opened {
        node: peer_id_of(v.get("node")?)?,
        port: port_of(v, "port")?,
        proto: proto_of(v)?,
        local: port_of(v, "local")?,
    })
}

pub(super) fn parse_opened_list(a: &[Value]) -> Option<Vec<Opened>> {
    let list = a.iter().map(opened_one).collect::<Option<Vec<_>>>()?;
    let mut pairs = HashSet::new();
    let mut locals = HashSet::new();
    let unique = list
        .iter()
        .all(|o| pairs.insert((o.node, o.proto, o.port)) & locals.insert((o.proto, o.local)));
    (list.len() <= MAX_PORT_LIST_ENTRIES && unique).then_some(list)
}

#[cfg(test)]
#[path = "parse_ports_tests.rs"]
mod tests;
