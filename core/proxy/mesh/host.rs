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
        let host = domain.trim_end_matches('.');
        if host == MESH_SUFFIX {
            return Some(MeshHost { label: String::new() });
        }
        let label = host.strip_suffix(&format!(".{MESH_SUFFIX}"))?;
        Some(MeshHost { label: label.to_string() })
    }
}

/// Link to a device address: port 80 is left out, port 443 selects `https`.
pub(crate) fn mesh_url(address: &str, port: u16) -> String {
    let scheme = if port == HTTPS_PORT { "https" } else { "http" };
    if port == HTTP_PORT || port == HTTPS_PORT {
        return format!("{scheme}://{address}.{MESH_SUFFIX}");
    }
    format!("{scheme}://{address}.{MESH_SUFFIX}:{port}")
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
