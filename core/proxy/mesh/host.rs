//! `.meshtown` host parsing and address links.

use crate::proxy::socks5::Target;

/// Last label of every host the node answers itself.
pub(crate) const MESH_SUFFIX: &str = "meshtown";
const HTTPS_PORT: u16 = 443;
const HTTP_PORT: u16 = 80;

/// A host that belongs to the node: everything before the last `.meshtown` label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MeshHost {
    /// May be empty or not a valid label; lookup then finds nothing.
    pub label: String,
}

impl MeshHost {
    /// `Some` for every domain whose last label is `meshtown`, valid remainder or not.
    pub(crate) fn parse(target: &Target) -> Option<MeshHost> {
        let Target::Domain(domain) = target else {
            return None;
        };
        mesh_label(domain).map(|label| MeshHost {
            label: label.to_string(),
        })
    }
}

/// The part of a host before the last `.meshtown` label (empty for the bare suffix); `None` for any other host.
pub(crate) fn mesh_label(host: &str) -> Option<&str> {
    let host = host.trim_end_matches('.');
    if host == MESH_SUFFIX {
        return Some("");
    }
    host.strip_suffix(MESH_SUFFIX)?.strip_suffix('.')
}

/// Whether the host's last label is `meshtown`.
pub(crate) fn is_mesh_name(host: &str) -> bool {
    mesh_label(host).is_some()
}

/// Link to a device address: port 80 is left out, port 443 selects `https`.
pub(crate) fn mesh_url(address: &str, port: u16) -> String {
    host_url(&format!("{address}.{MESH_SUFFIX}"), port)
}

/// Link to a full `<address>.meshtown` host, with the same port rules as [`mesh_url`].
pub(crate) fn host_url(host: &str, port: u16) -> String {
    let scheme = if port == HTTPS_PORT { "https" } else { "http" };
    if port == HTTP_PORT || port == HTTPS_PORT {
        return format!("{scheme}://{host}");
    }
    format!("{scheme}://{host}:{port}")
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
