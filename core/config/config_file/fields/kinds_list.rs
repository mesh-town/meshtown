//! List value kinds: nodes, publications and opened ports, network stack names.

use std::collections::HashSet;

use serde_json::Value;

use crate::node::net_names;
use crate::ports_sharing::types::{Audience, Opened, Published};
use crate::transport::net::{NetChoice, PeerId};

use super::super::parse_fields::push_field_error;
use super::super::parse_ports::{parse_opened_list, parse_published_list};
use super::super::report::FieldError;
use super::super::settings::Settings;
use super::row::Block;
use super::value::{FieldValue, Kind};

const MAX_PEERS: usize = 64;
const EXPECTED_PEERS: &str = "a list of node ids without repeats, up to 64";
const EXPECTED_PUBLISHED: &str =
    "a list of publications {port, name, for} without repeated ports, up to 64, for is \"all\" or a list of ids";
const EXPECTED_OPENED: &str =
    "a list of {node, port, local} without repeats of (node, port) and local, up to 64";

/// A list of node ids without repeats, up to 64; order is kept.
pub(crate) struct PeerList;

impl Kind<Vec<PeerId>> for PeerList {
    fn to_json(&self, v: &Vec<PeerId>) -> Option<Value> {
        Some(v.iter().map(ToString::to_string).collect())
    }
    fn parse(&self, v: &Value) -> Option<Vec<PeerId>> {
        let ids: Vec<PeerId> = v
            .as_array()?
            .iter()
            .map(|x| x.as_str().and_then(|s| s.parse::<PeerId>().ok()))
            .collect::<Option<_>>()?;
        let mut seen = HashSet::new();
        let unique = ids.iter().all(|id| seen.insert(*id));
        (ids.len() <= MAX_PEERS && unique).then_some(ids)
    }
    fn expected(&self) -> String {
        EXPECTED_PEERS.to_string()
    }
    fn default_text(&self, _: &Vec<PeerId>) -> String {
        "[]".to_string()
    }
}

/// `ports.published`: the list is either accepted whole or dropped whole.
pub(crate) struct PublishedList;

impl Kind<Vec<Published>> for PublishedList {
    fn to_json(&self, v: &Vec<Published>) -> Option<Value> {
        Some(v.iter().map(published_json).collect())
    }
    fn parse(&self, v: &Value) -> Option<Vec<Published>> {
        parse_published_list(v.as_array()?)
    }
    fn expected(&self) -> String {
        EXPECTED_PUBLISHED.to_string()
    }
    fn default_text(&self, _: &Vec<Published>) -> String {
        "[]".to_string()
    }
}

/// `ports.opened`: the list is either accepted whole or dropped whole.
pub(crate) struct OpenedList;

impl Kind<Vec<Opened>> for OpenedList {
    fn to_json(&self, v: &Vec<Opened>) -> Option<Value> {
        Some(v.iter().map(opened_json).collect())
    }
    fn parse(&self, v: &Value) -> Option<Vec<Opened>> {
        parse_opened_list(v.as_array()?)
    }
    fn expected(&self) -> String {
        EXPECTED_OPENED.to_string()
    }
    fn default_text(&self, _: &Vec<Opened>) -> String {
        "[]".to_string()
    }
}

/// A publication in the file: `port`, `name`, `for` (key order is set by `ConfigDoc`).
#[derive(serde::Serialize)]
struct PublishedOut {
    port: u16,
    name: String,
    #[serde(rename = "for")]
    audience: Value,
}

/// An opened port in the file: `node`, `port`, `local`.
#[derive(serde::Serialize)]
struct OpenedOut {
    node: String,
    port: u16,
    local: u16,
}

fn published_json(p: &Published) -> Value {
    let out = PublishedOut {
        port: p.port,
        name: p.name.clone(),
        audience: audience_json(&p.audience),
    };
    serde_json::to_value(out).unwrap_or(Value::Null)
}

fn opened_json(o: &Opened) -> Value {
    let out = OpenedOut {
        node: o.node.to_string(),
        port: o.port,
        local: o.local,
    };
    serde_json::to_value(out).unwrap_or(Value::Null)
}

/// `for` is always written: `"all"` or a list of ids as strings.
fn audience_json(a: &Audience) -> Value {
    match a {
        Audience::All => Value::from("all"),
        Audience::Nodes(ids) => ids.iter().map(ToString::to_string).collect(),
    }
}

/// The `net` part made of a non-empty list of names without repeats, all from `allowed`.
pub(crate) struct NetNames {
    pub names_in_choice: fn(&mut NetChoice) -> &mut Vec<String>,
    pub allowed: fn() -> Vec<&'static str>,
}

impl FieldValue for NetNames {
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    ) {
        let Some(v) = file_value else { return };
        let allowed = (self.allowed)();
        if let Some(list) = parse_allowed_names(v, &allowed) {
            *(self.names_in_choice)(&mut s.net.net_choice) = list;
            return;
        }
        let mut d = net_names::default_choice();
        push_field_error(
            errs,
            field_path.to_string(),
            v,
            &self.expected(),
            (self.names_in_choice)(&mut d).join(", "),
        );
    }

    fn expected(&self) -> String {
        format!(
            "a non-empty list without repeats from: {}",
            (self.allowed)().join(", ")
        )
    }

    fn copy_value_between(&self, from: &Settings, to: &mut Settings) {
        let mut src = from.net.net_choice.clone();
        *(self.names_in_choice)(&mut to.net.net_choice) = (self.names_in_choice)(&mut src).clone();
    }

    /// Written only when it differs from the build default.
    fn render_for_file(&self, s: &Settings) -> Option<Value> {
        let mut cur = s.net.net_choice.clone();
        let mut d = net_names::default_choice();
        let list = (self.names_in_choice)(&mut cur).clone();
        (list != *(self.names_in_choice)(&mut d)).then(|| list.into_iter().collect())
    }

    fn debug_text(&self, s: &Settings) -> String {
        format!(
            "{:?}",
            (self.names_in_choice)(&mut s.net.net_choice.clone())
        )
    }

    fn is_stored_in_settings(&self) -> bool {
        true
    }

    fn block(&self) -> Block {
        Block::Net
    }
}

fn parse_allowed_names(v: &Value, allowed: &[&str]) -> Option<Vec<String>> {
    let items = v.as_array().filter(|a| !a.is_empty())?;
    let list: Vec<String> = items
        .iter()
        .map(|i| i.as_str().filter(|s| allowed.contains(s)).map(String::from))
        .collect::<Option<_>>()?;
    let unique: HashSet<&String> = list.iter().collect();
    (unique.len() == list.len()).then_some(list)
}

/// `net.sync`: a string from `net_names::sync()`.
pub(crate) struct NetSync;

impl FieldValue for NetSync {
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    ) {
        let Some(v) = file_value else { return };
        let allowed = net_names::sync();
        if let Some(name) = v.as_str().filter(|n| allowed.contains(n)) {
            s.net.net_choice.sync = name.to_string();
            return;
        }
        push_field_error(
            errs,
            field_path.to_string(),
            v,
            &self.expected(),
            net_names::default_choice().sync,
        );
    }

    fn expected(&self) -> String {
        format!("one of: {}", net_names::sync().join(", "))
    }

    fn copy_value_between(&self, from: &Settings, to: &mut Settings) {
        to.net.net_choice.sync = from.net.net_choice.sync.clone();
    }

    /// Written only when it differs from the build default.
    fn render_for_file(&self, s: &Settings) -> Option<Value> {
        (s.net.net_choice.sync != net_names::default_choice().sync)
            .then(|| Value::from(s.net.net_choice.sync.as_str()))
    }

    fn debug_text(&self, s: &Settings) -> String {
        format!("{:?}", s.net.net_choice.sync)
    }

    fn is_stored_in_settings(&self) -> bool {
        true
    }

    fn block(&self) -> Block {
        Block::Net
    }
}

#[cfg(test)]
#[path = "kinds_list_tests.rs"]
mod tests;
