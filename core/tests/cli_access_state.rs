//! `access proxy` and `access ifaces` while the node restarts and on a node started without the proxy.

#[path = "common/cli_node.rs"]
mod cli_node;

use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
use meshtown_core::transport::net::NodeSecret;

use cli_node::{free_port, stderr as err, CliNode};

fn good_key() -> String {
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    managed_key_with_label(None, &base, &root)
}

/// `access proxy <value>`, repeated while the node asks to run the command again.
fn proxy_when_ready(node: &CliNode, value: &str) -> std::process::Output {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let o = node.cli(&["access", "proxy", value]);
        let busy = err(&o).contains("run the command again");
        if !busy || std::time::Instant::now() > deadline {
            return o;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

#[test]
fn proxy_change_while_starting_asks_to_run_again() {
    let port = free_port().to_string();
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{}"}}}}}}"#,
        good_key()
    );
    let node = CliNode::start_with_config(&config, &["--socks", "--socks-port", &port]);
    let widened = proxy_when_ready(&node, "localnet");
    assert_eq!(widened.status.code(), Some(0), "stderr: {}", err(&widened));
    // when: narrowed at once, before the node has come back
    let narrowed = node.cli(&["access", "proxy", "localhost"]);
    // then: never "unchanged"; the node either already restarts the proxy or asks to run the command again
    let text = err(&narrowed);
    assert!(!text.contains("unchanged"), "stderr: {text}");
    let advice = text.contains("run the command again") && narrowed.status.code() == Some(1);
    let restarting = text.contains("proxy restarting") && narrowed.status.code() == Some(0);
    assert!(advice || restarting, "stderr: {text}");
}

#[test]
fn proxy_change_on_no_socks_node_says_the_proxy_is_off() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["access", "proxy", "localnet"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("the proxy is off on this node"),
        "stderr: {}",
        err(&o)
    );
    let set = node.cli(&["config", "set", "socks.access", "localhost"]);
    assert_eq!(set.status.code(), Some(0), "stderr: {}", err(&set));
    assert!(
        !err(&set).contains("applies after"),
        "stderr: {}",
        err(&set)
    );
}

#[test]
fn ifaces_change_on_no_socks_node_says_the_proxy_is_off() {
    let node = CliNode::start(&[]);
    let o = node.cli(&["access", "ifaces", "br-lan", "lo"]);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        err(&o).contains("the proxy is off on this node"),
        "stderr: {}",
        err(&o)
    );
    assert!(
        !err(&o).contains("the proxy picks it up"),
        "stderr: {}",
        err(&o)
    );
}
