//! The receiving-interface filter (`socks.ifaces`, `web.ifaces`): three states and parsing of a name list.

const MAX_NAMES: usize = 64;
const MAX_NAME_BYTES: usize = 64;

/// `Auto` — the automatic logic decides (no key, `[]`, `null`); `Any` — `["*"]`, no restrictions;
/// `Only` — only the listed interfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum IfaceFilter {
    #[default]
    Auto,
    Any,
    Only(Vec<String>),
}

/// An interface name: non-empty, up to 64 bytes, no control characters, not `*` (which may only be alone in the list).
pub(crate) fn name_ok(name: &str) -> bool {
    let sized = !name.is_empty() && name.len() <= MAX_NAME_BYTES;
    sized && name != "*" && !name.chars().any(char::is_control)
}

fn names_ok(names: &[String]) -> bool {
    let unique = names.iter().collect::<std::collections::HashSet<_>>().len() == names.len();
    names.len() <= MAX_NAMES && unique && names.iter().all(|n| name_ok(n))
}

impl IfaceFilter {
    /// Name list → filter; `None` on any rejection (duplicate, empty name, `*` together with names…).
    pub fn from_names(names: Vec<String>) -> Option<IfaceFilter> {
        if names.is_empty() {
            return Some(IfaceFilter::Auto);
        }
        if names == ["*"] {
            return Some(IfaceFilter::Any);
        }
        names_ok(&names).then_some(IfaceFilter::Only(names))
    }

    /// The single output `match`: the kind label and the name list.
    pub fn parts(&self) -> (&'static str, Vec<String>) {
        match self {
            IfaceFilter::Auto => ("auto", Vec::new()),
            IfaceFilter::Any => ("any", vec!["*".to_string()]),
            IfaceFilter::Only(names) => ("list", names.clone()),
        }
    }

    /// For `/api/state`: `Auto` → `None` (automatic), otherwise the list.
    pub fn to_state(&self) -> Option<Vec<String>> {
        let (label, names) = self.parts();
        (label != "auto").then_some(names)
    }

    /// The inverse of `to_state`; a parse failure → `Auto`.
    pub fn from_state(names: Option<Vec<String>>) -> IfaceFilter {
        names.and_then(IfaceFilter::from_names).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "iface_filter_tests.rs"]
mod tests;
