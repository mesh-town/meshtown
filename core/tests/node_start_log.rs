//! The reference of log lines at node start.
//!
//! A separate integration binary: the log ring is shared by the process, and nodes in `lib` tests are
//! started by other tests without a lock, so their lines would fall into the window. Here the process has one node.
//! The test compares tags and the normalized text of lines, without the time field `t`.

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::output;
use meshtown_core::node::{self, NodeConfig};

/// Tags whose lines are part of the reference.
const TAGS: [&str; 8] = [
    "node", "net", "docs", "domains", "socks", "exit", "route", "guard",
];

/// Replaces the variable parts of a line with labels.
fn normalize(text: &str, node: &node::RunningNode) -> String {
    let full = node.id.to_string();
    let mut s = text
        .replace(&full, "<hex>")
        .replace(&node.id.fmt_short(), "<short>")
        .replace(&node.domains.shared_replica_id().fmt_short(), "<space>")
        .replace(&format!("\"{}\"", node.name_at_start), "\"<name>\"")
        .replace(env!("CARGO_PKG_VERSION"), "<ver>");
    // The network id in the banner (8 hex after the network name) isn't available in the public API, so we cut it in place.
    if let Some(at) = s.find("network Mesh Town ") {
        let from = at + "network Mesh Town ".len();
        s.replace_range(from..from + 8, "<net>");
    }
    s
}

#[tokio::test]
async fn start_log_order_matches_reference() {
    let after = output::log_page(0, 1).last_seq;
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let node = node::start_node(cfg).await.expect("node start");
    let page = output::log_page(after, 10_000);
    let got: Vec<String> = page
        .lines
        .iter()
        .filter(|l| TAGS.contains(&l.tag.as_str()))
        .map(|l| format!("[{}] {}", l.tag, normalize(&l.text, &node)))
        .collect();
    let want = [
        "[node] transports: mem",
        "[node] meshtown <ver>, network Mesh Town <net>, node <short> \"<name>\" (<hex>), identity temporary",
        "[node] Node identity is not saved, as set at startup. After a restart, peers will see a new node.",
        "[net] non-default network choice: transport mem; discovery invite, cache; sync mesh",
        "[docs] domain list: space <space>, author <short>",
        "[exit] serving as exit for members of network Mesh Town",
    ];
    node.shutdown().await;
    assert_eq!(got, want);
}
