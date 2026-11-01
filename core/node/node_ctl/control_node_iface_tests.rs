//! `NodeCtl::snapshot` reads the exit interface from the live node, not from the cached snapshot: a change
//! made through `set_exit_interface` shows at once, without waiting for the next collection.

use super::*;

use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;
use crate::members::network_key::NetworkKey;
use crate::node::{NodeConfig, RunningNode};
use crate::web_gui::NodeControl;

use super::snapshot::{collect, DocCache};

/// Attaches a fresh node to a controller whose cache is frozen with the node's current facts.
async fn frozen(name: Option<&str>) -> (std::sync::Arc<NodeCtl>, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let node = crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    node.set_exit_interface(name);
    let ctl = NodeCtl::new(&Settings::default());
    ctl.attach(&node);
    let collected = collect(&node, None, &mut DocCache::default()).await;
    let mut st = ctl
        .ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    st.snapshot_gen += 1;
    st.cache = Some(collected);
    drop(st);
    (ctl, node)
}

/// The interface of the snapshot as a comparable pair.
fn iface_of(ctl: &NodeCtl) -> Option<(String, IfaceState)> {
    ctl.snapshot().exit.interface.map(|i| (i.name, i.state))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_changed_interface_shows_at_once() {
    let (ctl, node) = frozen(None).await;
    let name = loopback_name();
    node.set_exit_interface(Some(&name));
    assert_eq!(iface_of(&ctl), Some((name, IfaceState::Up)));
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cleared_interface_shows_at_once() {
    let (ctl, node) = frozen(Some(&loopback_name())).await;
    node.set_exit_interface(None);
    assert_eq!(iface_of(&ctl), None);
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_interface_shows_as_missing() {
    let (ctl, node) = frozen(None).await;
    node.set_exit_interface(Some(MISSING_IFACE));
    assert_eq!(
        iface_of(&ctl),
        Some((MISSING_IFACE.to_string(), IfaceState::Missing))
    );
    node.shutdown().await;
}
