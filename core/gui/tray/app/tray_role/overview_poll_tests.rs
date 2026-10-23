//! Tests of polling the node: when the summary is asked for, the fallback to the state on 404, the retry on 401.

use super::*;
use crate::gui::tray::app::local_api::stub_node::{
    friends, overview_json, state_json, StateSpec, StubNode,
};
use crate::gui::tray::app::local_api::tests::{temp_dir, FAKE_TOKEN};
use serde_json::json;
use std::sync::Mutex;

mod gate {
    use super::*;

    #[test]
    fn fresh_gate_asks_for_the_summary() {
        assert!(OverviewGate::new().want_overview());
    }

    #[test]
    fn after_not_found_five_calls_skip_and_the_sixth_asks() {
        // given
        let mut gate = OverviewGate::new();

        // when
        gate.saw_not_found();

        // then
        let skipped: Vec<bool> = (0..5).map(|_| gate.want_overview()).collect();
        assert_eq!(skipped, [false; 5]);
        assert!(gate.want_overview(), "the sixth call asks again");
    }

    #[test]
    fn seeing_the_summary_clears_the_skips() {
        let mut gate = OverviewGate::new();
        gate.saw_not_found();
        assert!(!gate.want_overview());

        gate.saw_overview();

        assert!(gate.want_overview());
    }
}

mod fetch_node_reply {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn summary_answer_is_returned_without_asking_the_state() {
        // given
        let fx = setup().await;
        fx.node
            .reply("GET", OVERVIEW, 200, &overview_json(&[friends()]));

        // when
        let reply = fx.fetch().await;

        // then
        assert!(
            matches!(&reply, Ok(Reply::Ok(v)) if v["nets"].is_array()),
            "{reply:?}"
        );
        assert_eq!(fx.asked(), (1, 0));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn not_found_falls_back_to_the_state_in_the_same_call() {
        // given
        let fx = setup().await;

        // when
        let reply = fx.fetch().await;

        // then
        assert!(
            matches!(&reply, Ok(Reply::Ok(v)) if v["node"].is_object()),
            "{reply:?}"
        );
        assert_eq!(fx.asked(), (1, 1));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn after_not_found_five_calls_use_the_state_and_the_sixth_asks_the_summary() {
        // given
        let fx = setup().await;
        let _ = fx.fetch().await;

        // when
        for _ in 0..5 {
            let _ = fx.fetch().await;
        }
        let before = fx.asked().0;
        let _ = fx.fetch().await;

        // then
        assert_eq!(
            before, 1,
            "the five calls after the 404 did not ask the summary"
        );
        assert_eq!(fx.asked().0, 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn other_statuses_are_returned_as_they_are() {
        // given
        let fx = setup().await;
        fx.node
            .reply("GET", OVERVIEW, 500, &json!({ "error": "boom" }));

        // when
        let first = fx.fetch().await;
        let second = fx.fetch().await;

        // then
        assert!(matches!(first, Ok(Reply::OtherStatus(500))));
        assert!(matches!(second, Ok(Reply::OtherStatus(500))));
        assert_eq!(fx.asked(), (2, 0));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejected_token_is_retried_once() {
        // given
        let fx = setup().await;
        fx.node
            .reply_once("GET", OVERVIEW, 401, &json!({ "error": "bad_token" }));
        fx.node
            .reply("GET", OVERVIEW, 200, &overview_json(&[friends()]));

        // when
        let reply = fx.fetch().await;

        // then
        assert!(matches!(reply, Ok(Reply::Ok(_))));
        assert_eq!(fx.asked(), (2, 0));
    }
}

mod polling {
    use super::*;
    use crate::gui::tray::app::tray_role::poller::test_support::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn summary_node_is_asked_for_the_summary_on_every_tick() {
        // given
        let fx = setup().await;
        fx.node
            .reply("GET", OVERVIEW, 200, &overview_json(&[friends()]));
        let mut rig = Rig::start(fx.node.port(), fx.token_path(), true, spawn_panics());

        // when
        rig.probe.view("a ready view", is_ready);
        fx.node.wait_count("GET", OVERVIEW, 3).await;

        // then
        assert_eq!(fx.asked().1, 0, "the state is never asked");
        rig.quit();
        assert_eq!(rig.code().await, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn old_node_is_asked_for_the_summary_once_in_six_ticks() {
        // given
        let fx = setup().await;
        let mut rig = Rig::start(fx.node.port(), fx.token_path(), true, spawn_panics());
        rig.probe.view("a ready view", is_ready);

        // when
        fx.node.wait_count("GET", OVERVIEW, 2).await;

        // then
        assert!(
            fx.asked().1 >= 6,
            "five skipped ticks and the first fallback"
        );
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        assert_eq!(
            fx.asked().0,
            2,
            "no second summary request within the next ticks"
        );
        rig.quit();
        assert_eq!(rig.code().await, 0);
    }
}

const OVERVIEW: &str = "/api/overview";
const STATE: &str = "/api/state";

struct Fixture {
    node: StubNode,
    dir: std::path::PathBuf,
    gate: Mutex<OverviewGate>,
}

/// A stub node that knows `/api/state` only (an old node) and a token file.
async fn setup() -> Fixture {
    let node = StubNode::start().await;
    node.reply("GET", STATE, 200, &state_json(&StateSpec::default()));
    let dir = temp_dir("overview-poll");
    std::fs::write(dir.join("web-token"), FAKE_TOKEN).expect("write the token file");
    Fixture {
        node,
        dir,
        gate: Mutex::new(OverviewGate::new()),
    }
}

impl Fixture {
    fn token_path(&self) -> std::path::PathBuf {
        self.dir.join("web-token")
    }

    /// How many requests the stub got: (summary, state).
    fn asked(&self) -> (usize, usize) {
        (
            self.node.count("GET", OVERVIEW),
            self.node.count("GET", STATE),
        )
    }

    async fn fetch(&self) -> Result<Reply, ApiError> {
        let mut gate = OverviewGate::new();
        std::mem::swap(&mut gate, &mut *self.gate.lock().unwrap());
        let reply = fetch_node_reply(self.node.port(), &self.token_path(), &mut gate).await;
        std::mem::swap(&mut gate, &mut *self.gate.lock().unwrap());
        reply
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
