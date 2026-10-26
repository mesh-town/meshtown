//! A node that holds two networks, none of them online yet: the menu the tray gets keeps both submenus.

use crate::gui::tray::app::local_api::stub_node::StubNode;
use crate::gui::tray::app::local_api::tests::{temp_dir, FAKE_TOKEN};
use crate::gui::tray::app::menu_order::has_network;
use crate::gui::tray::app::tray_role::poller::test_support::*;
use serde_json::Value;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn two_offline_networks_keep_their_submenus_after_the_grey_view() {
    // given: the reply of a real node whose two networks are both offline
    let node = StubNode::start().await;
    let reply: Value =
        serde_json::from_str(include_str!("real_offline_overview_nodes.json")).unwrap();
    node.reply("GET", "/api/overview", 200, &reply);
    let dir = temp_dir("poller-offline-nets");
    std::fs::write(dir.join("web-token"), FAKE_TOKEN).expect("write the token file");

    // when: the poller has shown the view and polled a few more times
    let mut rig = Rig::start(node.port(), dir.join("web-token"), true, spawn_panics());
    rig.probe.view("a ready view", is_ready);
    std::thread::sleep(Duration::from_millis(500));
    while let Ok(item) = rig.probe.rx.try_recv() {
        rig.probe.history.push(item);
    }

    // then: the last menu sent to the tray still has both networks and the add item
    let last = rig.probe.history.iter().rev().find_map(|s| match s {
        Seen::Exits(m) => Some(m.clone()),
        _ => None,
    });
    let menu = last.expect("a menu was sent");
    assert_eq!(menu.nets.len(), 2);
    assert!(has_network(&menu));
    rig.quit();
    let _ = std::fs::remove_dir_all(&dir);
}
