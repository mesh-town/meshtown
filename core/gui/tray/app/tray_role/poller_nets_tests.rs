//! Tests of the commands sent to a node that holds several networks: the request paths carry the slot.

use crate::gui::tray::app::local_api::stub_node::{friends, overview_json, work, StubNode};
use crate::gui::tray::app::local_api::tests::{temp_dir, FAKE_TOKEN};
use crate::gui::tray::app::tray_actions::InviteNote;
use crate::gui::tray::app::tray_role::poller::test_support::*;
use crate::gui::tray::NodeCmd;
use serde_json::{json, Value};

mod copy_invite {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn network_invite_is_asked_under_its_slot() {
        // given
        let mut fx = setup().await;
        fx.node.reply(
            "GET",
            "/api/net/ab12cd/network/invite",
            200,
            &json!({ "invite": "INVITE-TEXT" }),
        );

        // when
        fx.rig.cmd(NodeCmd::CopyInvite(Some("ab12cd".into())));
        fx.rig
            .probe
            .invite("a copied note", |n| matches!(n, InviteNote::Copied(_)));

        // then
        assert_eq!(fx.rig.clip.lock().unwrap().copied, ["INVITE-TEXT"]);
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn without_a_slot_the_old_path_is_used() {
        // given
        let fx = setup().await;
        fx.node.reply(
            "GET",
            "/api/network/invite",
            200,
            &json!({ "invite": "INVITE-TEXT" }),
        );

        // when
        fx.rig.cmd(NodeCmd::CopyInvite(None));
        fx.node.wait_count("GET", "/api/network/invite", 1).await;

        // then
        assert_eq!(fx.node.count("GET", "/api/net/ab12cd/network/invite"), 0);
        fx.finish().await;
    }
}

mod exits {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_reads_and_writes_the_config_of_the_network() {
        // given
        let fx = setup().await;
        fx.config("/api/net/ab12cd/config", &[]);

        // when
        fx.rig.cmd(NodeCmd::ToggleExit {
            slot: Some("ab12cd".into()),
            id: "peerb".into(),
        });
        let put = fx.put("/api/net/ab12cd/config").await;

        // then
        assert_eq!(
            put,
            json!({ "rev": 3, "config": { "route": { "exits": ["peerb"] } } })
        );
        assert_eq!(
            fx.node.count("GET", "/api/config"),
            0,
            "the main network is not touched"
        );
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn clear_empties_the_exits_of_the_network() {
        // given
        let fx = setup().await;
        fx.config("/api/net/ab12cd/config", &["peerb"]);

        // when
        fx.rig.cmd(NodeCmd::ClearExits(Some("ab12cd".into())));
        let put = fx.put("/api/net/ab12cd/config").await;

        // then
        assert_eq!(
            put,
            json!({ "rev": 3, "config": { "route": { "exits": [] } } })
        );
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_without_a_slot_uses_the_old_path() {
        // given
        let fx = setup().await;
        fx.config("/api/config", &[]);

        // when
        fx.rig.cmd(NodeCmd::ToggleExit {
            slot: None,
            id: "peerb".into(),
        });
        let put = fx.put("/api/config").await;

        // then
        assert_eq!(put["config"]["route"]["exits"], json!(["peerb"]));
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serving_is_switched_in_the_config_of_the_network() {
        // given
        let fx = setup().await;
        fx.config("/api/net/ab12cd/config", &[]);

        // when
        fx.rig.cmd(NodeCmd::SetExitServer {
            slot: Some("ab12cd".into()),
            on: true,
        });
        let put = fx.put("/api/net/ab12cd/config").await;

        // then
        assert_eq!(
            put,
            json!({ "rev": 3, "config": { "exit": { "enabled": true } } })
        );
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serving_without_a_slot_uses_the_old_path() {
        // given
        let fx = setup().await;
        fx.config("/api/config", &[]);

        // when
        fx.rig.cmd(NodeCmd::SetExitServer {
            slot: None,
            on: false,
        });
        let put = fx.put("/api/config").await;

        // then
        assert_eq!(put["config"]["exit"]["enabled"], json!(false));
        fx.finish().await;
    }
}

struct Fixture {
    node: StubNode,
    rig: Rig,
    dir: std::path::PathBuf,
}

/// A node with two networks that is already shown as ready.
async fn setup() -> Fixture {
    let node = StubNode::start().await;
    node.reply(
        "GET",
        "/api/overview",
        200,
        &overview_json(&[friends(), work()]),
    );
    let dir = temp_dir("poller-nets");
    std::fs::write(dir.join("web-token"), FAKE_TOKEN).expect("write the token file");
    let mut rig = Rig::start(node.port(), dir.join("web-token"), true, spawn_panics());
    rig.probe.view("a ready view", is_ready);
    Fixture { node, rig, dir }
}

impl Fixture {
    /// The config document `path` answers with: revision 3 and the selected exits.
    fn config(&self, path: &str, exits: &[&str]) {
        let doc = json!({ "rev": 3, "config": { "route": { "exits": exits } } });
        self.node.reply("GET", path, 200, &doc);
        self.node.reply("PUT", path, 200, &json!({}));
    }

    /// The body of the first `PUT path`, once it has arrived.
    async fn put(&self, path: &str) -> Value {
        self.node.wait_count("PUT", path, 1).await;
        serde_json::from_str(&self.node.hits("PUT", path)[0].body).expect("a JSON body")
    }

    async fn finish(mut self) {
        self.rig.quit();
        assert_eq!(self.rig.code().await, 0);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
