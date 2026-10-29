//! The macOS LaunchAgent property list.

use std::path::{Path, PathBuf};

/// The label used when the binary does not run from an app bundle.
pub const FALLBACK_LABEL: &str = "local.meshtown";

/// How the agent starts the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacLaunch {
    /// Open the `.app` through LaunchServices.
    Open(PathBuf),
    /// Run the binary with the `tray` argument.
    Exe(PathBuf),
}

/// Label and launch method: from a bundle `(app path, bundle id)` or from a bare binary.
pub fn mac_launch(bundle: Option<(&Path, &str)>, exe: &Path) -> (String, MacLaunch) {
    match bundle {
        Some((app, id)) => (id.to_string(), MacLaunch::Open(app.to_path_buf())),
        None => (
            FALLBACK_LABEL.to_string(),
            MacLaunch::Exe(exe.to_path_buf()),
        ),
    }
}

/// `home/Library/LaunchAgents/<label>.plist`.
pub fn launch_agent_path(home: &Path, label: &str) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"))
}

/// The plist text for the agent.
pub fn launch_agent_plist(label: &str, launch: &MacLaunch) -> String {
    let args = program_arguments(launch)
        .iter()
        .map(|a| format!("\t\t<string>{}</string>\n", xml_escape(a)))
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
\t<key>Label</key>\n\
\t<string>{}</string>\n\
\t<key>ProgramArguments</key>\n\
\t<array>\n\
{args}\
\t</array>\n\
\t<key>RunAtLoad</key>\n\
\t<true/>\n\
\t<key>LimitLoadToSessionType</key>\n\
\t<string>Aqua</string>\n\
</dict>\n\
</plist>\n",
        xml_escape(label)
    )
}

fn program_arguments(launch: &MacLaunch) -> Vec<String> {
    match launch {
        MacLaunch::Open(app) => vec!["/usr/bin/open".into(), "-a".into(), lossy(app)],
        MacLaunch::Exe(exe) => vec![lossy(exe), "tray".into()],
    }
}

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    text.chars().for_each(|c| match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&apos;"),
        other => out.push(other),
    });
    out
}

/// The plist has `<key>Disabled</key>` followed by `<true/>`.
pub fn launch_agent_switched_off(text: &str) -> bool {
    const KEY: &str = "<key>Disabled</key>";
    text.find(KEY)
        .is_some_and(|at| text[at + KEY.len()..].trim_start().starts_with("<true/>"))
}

#[cfg(test)]
#[path = "launch_agent_tests.rs"]
mod tests;
