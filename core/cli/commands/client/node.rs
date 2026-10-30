//! A running node as the commands see it: the page, the network chosen with `--net`, and requests that
//! turn a refusal into an `ApiError` and a missing node into `NotRunning`.

use serde_json::{json, Value};

use super::api_error::ApiError;
use super::{LinkError, Target};
use crate::cli::args::Globals;
use crate::cli::exit::{NotRunning, UsageError};
use crate::cli::program_name::program_name;

/// A request function: method, path and body; the reply body or an error.
type Call<'a> = dyn FnMut(&str, &str, Option<&Value>) -> anyhow::Result<Value> + 'a;

/// The node found on its page port.
pub(crate) struct Node {
    pub target: Target,
    /// `/api`, or `/api/net/{slot}` when `--net` picked a network.
    pub base: String,
    /// The reply of `GET /api/nets` taken when connecting: `{"nets": [...]}`.
    pub nets: Value,
    /// Whether `--net` was typed.
    pub explicit_net: bool,
}

impl Node {
    /// Finds the node: `GET /api/nets`, then the network named by `--net`. No answer is `NotRunning`.
    pub(crate) fn connect(g: &Globals) -> anyhow::Result<Node> {
        let target = Target::resolve(g.config.clone(), g.web_port).map_err(anyhow::Error::msg)?;
        let nets = request(&target, "GET", "/api/nets", None)?;
        let base = match g.net.as_deref() {
            Some(wanted) => net_base(&nets, wanted)?,
            None => "/api".to_string(),
        };
        Ok(Node {
            target,
            base,
            nets,
            explicit_net: g.net.is_some(),
        })
    }

    /// The path of `tail` under the chosen network: `{base}/{tail}`.
    pub(crate) fn net_path(&self, tail: &str) -> String {
        format!("{}/{tail}", self.base)
    }

    /// One request. The reply body, `Null` when it is empty; a status other than 2xx is an `ApiError`.
    pub(crate) fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<Value> {
        request(&self.target, method, path, body)
    }

    /// A command on one network needs `--net` when the computer has several.
    pub(crate) fn require_net(&self, command: &str) -> anyhow::Result<()> {
        let count = net_list(&self.nets).len();
        match (count > 1, self.explicit_net) {
            (true, false) => Err(anyhow::Error::new(UsageError(format!(
                "this computer has {count} networks; choose one with --net <NETWORK>: {}. See: {} {command} --help",
                describe_nets(&self.nets),
                program_name()
            )))),
            _ => Ok(()),
        }
    }

    /// Changes settings: reads the revision, writes `patch`; a stale revision is retried once.
    pub(crate) fn put_config(&self, patch: &Value) -> anyhow::Result<Value> {
        let mut call =
            |method: &str, path: &str, body: Option<&Value>| self.call(method, path, body);
        put_with_retry(&mut call, &self.base, patch)
    }
}

/// One request to the page, with the token of the settings directory.
fn request(
    target: &Target,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> anyhow::Result<Value> {
    let text = body.map(Value::to_string);
    match target.call(method, path, text.as_deref()) {
        Ok((status, reply)) => reply_value(status, &reply),
        Err(LinkError::NotRunning) => Err(anyhow::Error::new(NotRunning { port: target.port })),
        Err(LinkError::Other(why)) => Err(anyhow::Error::msg(why)),
    }
}

/// A 2xx reply as JSON (an empty body is `Null`); anything else as an `ApiError`.
fn reply_value(status: u16, reply: &str) -> anyhow::Result<Value> {
    match (status / 100, reply.trim().is_empty()) {
        (2, true) => Ok(Value::Null),
        (2, false) => {
            serde_json::from_str(reply).map_err(|e| anyhow::anyhow!("the reply is not JSON: {e}"))
        }
        _ => Err(anyhow::Error::new(ApiError::from_reply(status, reply))),
    }
}

/// The networks of a `GET /api/nets` reply (or the bare list).
fn net_list(nets: &Value) -> &[Value] {
    let list = nets.get("nets").unwrap_or(nets);
    list.as_array().map_or(&[], Vec::as_slice)
}

/// `label (id)` of each network, for the lists in refusals.
fn describe_nets(nets: &Value) -> String {
    let one = |n: &Value| {
        let label = n["label"].as_str().unwrap_or("(no label)");
        format!("{label} ({})", n["id"].as_str().unwrap_or("?"))
    };
    net_list(nets)
        .iter()
        .map(one)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `/api` for the main network, `/api/net/{slot}` for the others.
fn net_base(nets: &Value, wanted: &str) -> anyhow::Result<String> {
    let slot = pick_net(nets, wanted).map_err(anyhow::Error::msg)?;
    Ok(format!("/api/net/{slot}"))
}

/// The slot of the network `wanted` names: the slot id exactly, then the label ignoring case, then the start
/// of `id_hex` (at least four characters). Two matches or none give the text of a refusal with the list.
pub(crate) fn pick_net(nets: &Value, wanted: &str) -> Result<String, String> {
    let list = net_list(nets);
    let by_id = |n: &Value| n["id"].as_str() == Some(wanted);
    let by_label = |n: &Value| {
        n["label"]
            .as_str()
            .is_some_and(|l| l.to_lowercase() == wanted.to_lowercase())
    };
    let by_hex = |n: &Value| {
        let hex = n["id_hex"].as_str().unwrap_or("");
        wanted.chars().count() >= 4 && hex.to_lowercase().starts_with(&wanted.to_lowercase())
    };
    let steps: [&dyn Fn(&Value) -> bool; 3] = [&by_id, &by_label, &by_hex];
    let found = steps
        .iter()
        .map(|f| list.iter().filter(|n| f(n)).collect::<Vec<_>>());
    match found.into_iter().find(|hits| !hits.is_empty()) {
        Some(hits) if hits.len() == 1 => Ok(hits[0]["id"].as_str().unwrap_or("main").to_string()),
        Some(_) => Err(format!(
            "\"{wanted}\" matches several networks: {}",
            describe_nets(nets)
        )),
        None => Err(format!(
            "no network matches \"{wanted}\"; networks: {}",
            describe_nets(nets)
        )),
    }
}

/// The percent-encoding of a query value: `A-Za-z0-9-._~` as they are, every other byte of the UTF-8 text as `%XX`.
pub(crate) fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `GET {base}/config` for the revision, then `PUT {base}/config`; a `stale_rev` answer is retried once with
/// the revision it carries, a second one is returned.
pub(super) fn put_with_retry(
    call: &mut Call<'_>,
    base: &str,
    patch: &Value,
) -> anyhow::Result<Value> {
    let path = format!("{base}/config");
    let current = call("GET", &path, None)?;
    let rev = current["rev"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("the reply has no settings revision"))?;
    let mut put = |rev: u64| call("PUT", &path, Some(&json!({"rev": rev, "config": patch})));
    match put(rev) {
        Err(e) => match stale_rev(&e) {
            Some(fresh) => put(fresh),
            None => Err(e),
        },
        done => done,
    }
}

/// The revision a `stale_rev` refusal carries.
fn stale_rev(e: &anyhow::Error) -> Option<u64> {
    let refused = e.chain().find_map(|c| c.downcast_ref::<ApiError>())?;
    match refused.code.as_str() {
        "stale_rev" => refused.body["rev"].as_u64(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
