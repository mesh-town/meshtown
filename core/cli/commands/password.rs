//! The `password reset` subcommand: resets the node page password. If the node is running, it asks the node through
//! `POST /api/auth/reset` and prints the addresses with a temporary token; if not, it deletes the password file itself.
//! Tokens and addresses with a token go only through `println!`, not `output::say` (which writes to the `/api/log` log).

use serde_json::Value;

use super::client::{LinkError, Target};
use crate::cli::args::Globals;
use crate::node::output::NET_NAME;

/// Entry point from `main`: finds the node and resets the password.
pub(crate) async fn run_password_command(g: &Globals) -> anyhow::Result<()> {
    g.allow("password", &[])?;
    let target = Target::resolve(g.config.clone(), g.web_port).map_err(anyhow::Error::msg)?;
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
        return format!("{NET_NAME} refused (code {code}): no web-token file next to the settings file {}; a password reset only works with the permanent token", t.path.display());
    }
    let head: String = body.chars().take(200).collect();
    format!("{NET_NAME} refused the password reset (code {code}): {head}")
}

fn remove_offline(t: &Target) -> Result<(), String> {
    let existed = crate::web_gui::take_password_file(&t.dir)
        .map_err(|e| format!("password file was not removed: {e}"))?;
    if !existed {
        return Err(format!(
            "no page password file in {}, nothing to reset; if {NET_NAME} keeps its settings elsewhere, pass --config <PATH>",
            t.dir.display()
        ));
    }
    println!("Password reset. {NET_NAME} isn't running: it will print a one-time sign-in link when it starts.");
    println!("If {NET_NAME} runs on another page port, repeat with --web-port <PORT>.");
    Ok(())
}
