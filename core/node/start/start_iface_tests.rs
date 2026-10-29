//! The exit's outgoing interface on a started node: the setting survives the start, and the live
//! handle (`exit.interface`, applied the way a settings change reaches a running node) updates the
//! node and its exit resolver.

use super::*;
use crate::config::config_file::{NetSettings, Settings};
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::node::node_ctl::NodeCtl;
use crate::web_gui::{NodeControl, NodeStatus};

mod exit_interface_handle_updates_node {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sets_the_name() {
        // given: an attached node with no interface
        let (ctl, node) = attached_node().await;
        // when: the path is applied with a name
        ctl.apply_config(&settings_with(Some(&loopback_name())), &[PATH]);
        // then
        assert_eq!(current(&node), Some(loopback_name()));
        finish(ctl, node).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clears_the_name() {
        // given: a node with an interface set through the handle
        let (ctl, node) = attached_node().await;
        ctl.apply_config(&settings_with(Some(&loopback_name())), &[PATH]);
        // when: the path is applied with `None`
        ctl.apply_config(&settings_with(None), &[PATH]);
        // then
        assert_eq!(current(&node), None);
        finish(ctl, node).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_exit_resolver_sees_the_new_name() {
        // given: an attached node
        let (ctl, node) = attached_node().await;
        // when
        ctl.apply_config(&settings_with(Some(MISSING_IFACE)), &[PATH]);
        // then: the resolver is bound to the very value the exit reads
        let resolver_iface = node
            .exit_resolver
            .iface()
            .expect("the exit resolver is bound");
        assert_eq!(resolver_iface.current().as_deref(), Some(MISSING_IFACE));
        finish(ctl, node).await;
    }

    #[cfg(feature = "net-hyper")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_change_empties_the_exit_cache() {
        // given: an address cached by the exit resolver
        let (ctl, node) = attached_node().await;
        node.exit_resolver
            .seed_cache_for_tests("site.test", vec![[192, 0, 2, 1].into()]);
        // when: the interface changes
        ctl.apply_config(&settings_with(Some(MISSING_IFACE)), &[PATH]);
        // then: answers fetched through the old interface are gone
        assert_eq!(node.exit_resolver.cached_for_tests("site.test"), None);
        finish(ctl, node).await;
    }

    #[cfg(feature = "net-hyper")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_same_value_keeps_the_exit_cache() {
        // given: an interface already set and an address cached after that
        let (ctl, node) = attached_node().await;
        ctl.apply_config(&settings_with(Some(MISSING_IFACE)), &[PATH]);
        node.exit_resolver
            .seed_cache_for_tests("site.test", vec![[192, 0, 2, 1].into()]);
        // when: the same value is applied again
        ctl.apply_config(&settings_with(Some(MISSING_IFACE)), &[PATH]);
        // then: nothing changed, so nothing is dropped
        assert!(node.exit_resolver.cached_for_tests("site.test").is_some());
        finish(ctl, node).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_same_value_reports_no_change() {
        // given: an interface set through the handle
        let (ctl, node) = attached_node().await;
        ctl.apply_config(&settings_with(Some(MISSING_IFACE)), &[PATH]);
        // when / then: setting it again is not a change, a different one is
        assert!(!node.exit_iface.set(Some(MISSING_IFACE)));
        assert!(node.exit_iface.set(None));
        finish(ctl, node).await;
    }
}

mod start_keeps_exit_interface {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_configured_name_is_on_the_node() {
        // given: settings that name an interface the machine does not have
        let mut cfg = loopback_cfg();
        cfg.settings.exit_interface = Some(MISSING_IFACE.to_string());
        // when: the node starts
        let node = start_node(cfg).await.unwrap();
        // then: the start does not drop it (nor refuse to start)
        assert_eq!(current(&node).as_deref(), Some(MISSING_IFACE));
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn no_setting_means_any_network() {
        // given / when
        let node = loopback_node().await;
        // then
        assert_eq!(current(&node), None);
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_exit_resolver_starts_bound_to_the_same_value() {
        // given: a named interface in the settings
        let mut cfg = loopback_cfg();
        cfg.settings.exit_interface = Some(MISSING_IFACE.to_string());
        // when
        let node = start_node(cfg).await.unwrap();
        // then
        let resolver_iface = node
            .exit_resolver
            .iface()
            .expect("the exit resolver is bound");
        assert_eq!(resolver_iface.current().as_deref(), Some(MISSING_IFACE));
        node.shutdown().await;
    }
}

const PATH: &str = "exit.interface";

/// A loopback node attached to a control object, the way the host runs it.
async fn attached_node() -> (std::sync::Arc<NodeCtl>, RunningNode) {
    let ctl = NodeCtl::new(&Settings::default());
    let node = loopback_node().await;
    ctl.attach(&node);
    (ctl, node)
}

async fn finish(ctl: std::sync::Arc<NodeCtl>, node: RunningNode) {
    ctl.detach(NodeStatus::Stopping);
    node.shutdown().await;
}

fn settings_with(name: Option<&str>) -> Settings {
    Settings {
        net: NetSettings {
            exit_interface: name.map(str::to_string),
            ..NetSettings::default()
        },
        ..Settings::default()
    }
}

fn current(node: &RunningNode) -> Option<String> {
    node.exit_iface.current().map(|n| n.to_string())
}
