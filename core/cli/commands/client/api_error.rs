//! A refusal from the node: the HTTP status, the error code and the message of the reply, and what to do next.

use std::fmt;

use serde_json::Value;

use crate::cli::program_name::program_name;
use crate::node::output::NET_NAME;

/// A reply with a status other than 2xx. It sits in the chain of the returned error (exit code 1).
#[derive(Debug)]
pub(crate) struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub body: Value,
}

impl ApiError {
    /// Reads `error` and `message` from the body of the reply; a body that isn't JSON leaves both empty.
    pub(crate) fn from_reply(status: u16, body: &str) -> ApiError {
        let body: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let text = |key: &str| body[key].as_str().unwrap_or("").to_string();
        ApiError {
            status,
            code: text("error"),
            message: text("message"),
            body,
        }
    }

    /// What to do next, from the code of the reply or, for a refused sign-in, from the status.
    fn advice(&self) -> Option<String> {
        match (refusal_advice(&self.code), self.status) {
            (Some(advice), _) => Some(advice),
            (None, 401) => Some(
                "Run it as the user that runs Mesh Town, or pass --config.".to_string(),
            ),
            _ => None,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self.message.is_empty() {
            true => format!("{NET_NAME} answered HTTP {}", self.status),
            false => self.message.clone(),
        };
        write!(f, "{}.", text.trim_end_matches('.'))?;
        match self.advice() {
            Some(advice) => write!(f, " {advice}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for ApiError {}

/// What to do after a refusal, by the code of the reply; `None` for a code that needs no advice.
pub(crate) fn refusal_advice(code: &str) -> Option<String> {
    let prog = program_name();
    let text = match code {
        "not_admin" => "Ask an administrator of this network.".to_string(),
        "self_target" => format!("Pick another member from `{prog} member list`."),
        "not_member" | "left" => format!("Run `{prog} member list` to see who is in the network."),
        "not_pending" => format!(
            "Waiting members have the status `pending` in `{prog} member list`."
        ),
        "last_admin" => format!(
            "Make another member an administrator first: `{prog} member admin <member>`."
        ),
        "network_deleted" | "unknown_net" => format!("See `{prog} network list`."),
        "network_present" => format!("Pick another network with --net, or see `{prog} network list`."),
        "net_already_added" => format!("This network is already on this computer: `{prog} network list`."),
        "too_many_nets" => format!("Remove a network first: `{prog} leave --net <network> --yes`."),
        "busy" => "Try again in a moment.".to_string(),
        "stopping" => format!("{NET_NAME} is shutting down; start it again when it has stopped."),
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
#[path = "api_error_tests.rs"]
mod tests;
