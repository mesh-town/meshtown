//! The display name of a network.

/// The name a network goes by in messages and menus: its label when not empty, else `Network` and the first
/// four characters of its id; `None` when there is neither.
pub fn net_display_name(label: Option<&str>, id_hex: Option<&str>) -> Option<String> {
    let label = label.filter(|l| !l.is_empty());
    if let Some(label) = label {
        return Some(label.to_string());
    }
    id_hex.map(|id| format!("Network {}", id.chars().take(4).collect::<String>()))
}

#[cfg(test)]
#[path = "net_name_tests.rs"]
mod tests;
