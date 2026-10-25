//! Tests of `invite::make` on a stack that was closed: no invite is built and the refusal names the network.
//! Child module of `invite_tests.rs`.

use crate::invite::make;
use crate::members::network_key::NetworkKey;
use crate::node::output::NET_NAME;
use crate::node::{start_node, NodeConfig};

#[tokio::test]
async fn a_closed_stack_makes_no_invite() {
    // given: a running node whose stack was closed
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let node = start_node(cfg).await.unwrap();
    node.stack.close().await;
    // when
    let Err(error) = make(&node.invite_source(), None) else {
        panic!("a closed stack must not make an invite");
    };
    // then: the refusal names the network and the restart
    let text = error.to_string();
    assert!(
        text.contains(&format!("{NET_NAME} was restarted")),
        "unexpected refusal: {text}"
    );
    assert!(!text.contains("node"), "unexpected refusal: {text}");
    node.shutdown().await;
}
