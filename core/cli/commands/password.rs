//! The `password reset` subcommand: resets the node page password. If the node is running, it asks the node through
//! `POST /api/auth/reset` and prints the addresses with a temporary token; if not, it deletes the password file itself.
//! Tokens and addresses with a token go only through `println!`, not `output::say` (which writes to the `/api/log` log).

use serde_json::Value;

use super::node_link::{LinkError, Target};

/// Entry point from `main`: finds the node and resets the password.
pub(crate) fn run_password_command(
    config: Option<std::path::PathBuf>,
    web_port: Option<u16>,
) -> anyhow::Result<()> {
    let target = Target::resolve(config, web_port).map_err(anyhow::Error::msg)?;
    reset(&target).map_err(anyhow::Error::msg)
}

/// Resets the password on a live node or on disk.
fn reset(t: &Target) -> Result<(), String> {
    match t.call("POST", "/api/auth/reset", Some("{}")) {
        Ok((200, body)) => {
            print_urls(&body);
            Ok(())
        }
        Ok((code, body)) => Err(refusal(t, code, &body)),
        Err(LinkError::NotRunning) => remove_offline(t),
        Err(LinkError::Other(e)) => Err(e),
    }
}

fn print_urls(body: &str) {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    for url in json["urls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        println!("{url}");
    }
    println!("Password reset. The link works once, for 10 minutes.");
}

fn refusal(t: &Target, code: u16, body: &str) -> String {
    if t.token().is_empty() {
        return format!("node refused (code {code}): no web-token file next to the settings file {}; a password reset only works with the permanent token", t.path.display());
    }
    let head: String = body.chars().take(200).collect();
    format!("node refused the password reset (code {code}): {head}")
}

fn remove_offline(t: &Target) -> Result<(), String> {
    crate::web_gui::remove_password_file(&t.dir)
        .map_err(|e| format!("password file was not removed: {e}"))?;
    println!("Password reset. The node isn't running: it will print a one-time sign-in link when it starts.");
    println!("If the node runs on another page port, repeat with --web-port <PORT>.");
    Ok(())
}
