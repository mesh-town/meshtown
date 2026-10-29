//! The Linux autostart `.desktop` entry.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `$XDG_CONFIG_HOME` (absolute only) or `$HOME/.config`, then `autostart/meshtown.desktop`.
pub fn desktop_entry_path(var: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let set = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = set("XDG_CONFIG_HOME")
        .filter(|p| p.is_absolute())
        .or_else(|| set("HOME").map(|home| home.join(".config")))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is set")?;
    Ok(base.join("autostart").join("meshtown.desktop"))
}

/// One `Exec` argument: quoted per the Desktop Entry Specification, then escaped as a string value.
pub fn desktop_exec_arg(arg: &str) -> Result<String, String> {
    if arg.contains(['\n', '\r']) {
        return Err("the path contains a line break".to_string());
    }
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        quoted.push_str(&escape_char(c));
    }
    quoted.push('"');
    Ok(quoted)
}

fn escape_char(c: char) -> String {
    match c {
        '"' | '`' | '$' => format!("\\\\{c}"),
        '\\' => "\\\\\\\\".to_string(),
        '%' => "%%".to_string(),
        other => other.to_string(),
    }
}

/// The text of the entry that starts `exe tray` at login.
pub fn desktop_entry(exe: &Path) -> Result<String, String> {
    let exe = exe
        .to_str()
        .ok_or("the path of the binary is not valid UTF-8")?;
    let exec = desktop_exec_arg(exe)?;
    Ok(format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name=Mesh Town\n\
Comment=Start Mesh Town at login\n\
Exec={exec} tray\n\
Terminal=false\n\
X-GNOME-Autostart-enabled=true\n"
    ))
}

/// The user turned the entry off (`Hidden=true` or `X-GNOME-Autostart-enabled=false`).
pub fn desktop_switched_off(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .any(|l| l == "Hidden=true" || l == "X-GNOME-Autostart-enabled=false")
}

#[cfg(test)]
#[path = "desktop_file_tests.rs"]
mod tests;
