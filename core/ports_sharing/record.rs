//! Key and value of a publication entry in the shared document: `port/<proto>/<port>/` and postcard
//! `PortRecord`. Pure functions without a network — checked by `record_tests.rs`.

use serde::{Deserialize, Serialize};

use crate::transport::net::PeerId;

use super::types::{Audience, Proto, Published, MAX_NAME_CHARS, MAX_PORT_LIST_ENTRIES};

/// Version of the entry value layout.
pub const PORT_RECORD_VERSION: u8 = 1;

const PORT_PREFIXES: [&str; 2] = ["port/tcp/", "port/udp/"];

/// Prefix of the publication keys of a protocol: `port/tcp/` or `port/udp/`.
pub fn port_prefix(proto: Proto) -> &'static str {
    PORT_PREFIXES[proto as usize]
}

/// Entry key `port/<proto>/<port>/`; the trailing `/` is required: `del` removes all keys with
/// the prefix, and `port/tcp/80` without a slash would remove `8080` too.
pub fn port_key(proto: Proto, port: u16) -> Vec<u8> {
    format!("{}{port}/", port_prefix(proto)).into_bytes()
}

/// Protocol and port from a key; strict: no leading zeros or signs, 0 and foreign keys give `None`.
pub fn port_from_key(key: &[u8]) -> Option<(Proto, u16)> {
    let text = std::str::from_utf8(key).ok()?;
    Proto::ALL.into_iter().find_map(|proto| {
        let digits = text.strip_prefix(port_prefix(proto))?.strip_suffix('/')?;
        let port: u16 = digits.parse().ok()?;
        (port != 0 && port_key(proto, port) == key).then_some((proto, port))
    })
}

/// Entry value in the document. postcard 1.1.3 layout: `v` — 1 byte; `name` — a varint of
/// the length in bytes and UTF-8; `audience` — a varint of the variant number: `0` = `All`, nothing after it;
/// `1` = `Nodes`, a varint of the key count and 32 bytes per key. Example
/// `{ v: 1, name: "éøü", audience: All }` → `01 06 C3 A9 C3 B8 C3 BC 00`.
///
/// Compatibility: (1) a new field goes only at the end while `v = 1`; an old node does not see it,
/// and a new one must also read a layout without the tail, otherwise it would hide the entries of old nodes;
/// (2) narrowing the circle of recipients is done only by a new `Audience` variant (an old node
/// hides the entry), not by a field in the tail (an old one would show it to everyone); (3) an incompatible layout is
/// `v = 2`, nodes with `v = 1` hide such entries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortRecord {
    pub v: u8,
    pub name: String,
    pub audience: Audience,
}

impl From<&Published> for PortRecord {
    fn from(p: &Published) -> Self {
        PortRecord {
            v: PORT_RECORD_VERSION,
            name: p.name.clone(),
            audience: p.audience.clone(),
        }
    }
}

/// Bytes of the entry value.
pub fn encode_port_record(r: &PortRecord) -> Vec<u8> {
    postcard::to_stdvec(r).expect("PortRecord has no serialization errors")
}

/// Parses the value; `None` on any error, a foreign version and exceeding the limits.
/// Bytes after `audience` are discarded.
pub fn decode_port_record(bytes: &[u8]) -> Option<PortRecord> {
    let r: PortRecord = postcard::from_bytes(bytes).ok()?;
    let within = r.name.chars().count() <= MAX_NAME_CHARS && audience_fits(&r.audience);
    (r.v == PORT_RECORD_VERSION && within).then_some(r)
}

impl Audience {
    /// Whether the node is in the recipient circle: `All` — anyone, `Nodes` — only those in the list.
    /// The entry in the shared replica is not hidden by this: the protection is `admit_port_request` on the publishing node.
    pub fn includes(&self, node: &PeerId) -> bool {
        match self {
            Audience::All => true,
            Audience::Nodes(ids) => ids.contains(node),
        }
    }
}

impl PortRecord {
    /// Entry name for node `node` if the entry is open to it; otherwise `None`.
    pub fn name_for(&self, node: &PeerId) -> Option<&str> {
        self.audience.includes(node).then_some(self.name.as_str())
    }
}

fn audience_fits(a: &Audience) -> bool {
    match a {
        Audience::All => true,
        Audience::Nodes(ids) => ids.len() <= MAX_PORT_LIST_ENTRIES,
    }
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
