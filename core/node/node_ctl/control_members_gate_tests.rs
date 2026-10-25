//! The gates of the membership list in front of `NodeCtl::members` and `member_action`: which phase of the
//! control reads the list and which phase answers "busy". The list is readable as soon as the node is up;
//! the actions also wait for the first snapshot.

use super::*;
use crate::config::config_file::Settings;
use crate::node::output::NET_NAME;
use crate::web_gui::{MemberRow, NodeControl};

mod a_node_without_a_network {
    use super::*;

    #[tokio::test]
    async fn the_list_is_busy_without_a_network() {
        let fx = setup(Given::NoNetwork).await;

        let list = fx.when_list().await.unwrap_err();
        let act = fx.when_leave().await.unwrap_err();

        let prefix = "this computer has not joined";
        assert!(busy_text(&list).starts_with(prefix), "{list:?}");
        assert!(busy_text(&act).starts_with(prefix), "{act:?}");
    }
}

mod a_node_that_is_stopping {
    use super::*;

    #[tokio::test]
    async fn the_list_is_busy_while_stopping() {
        let fx = setup(Given::Stopping).await;

        let list = fx.when_list().await.unwrap_err();
        let act = fx.when_leave().await.unwrap_err();
        let exit = fx.ctl.find_exit("example.com").unwrap_err();

        let stopping = format!("{NET_NAME} is stopping");
        assert_eq!(busy_text(&list), stopping, "the list");
        assert_eq!(busy_text(&act), stopping, "the action");
        assert_eq!(busy_text_of(&exit), stopping, "the domain action");
    }
}

mod a_node_that_is_up_without_the_first_snapshot {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_list_is_read_before_the_first_snapshot() {
        let fx = setup(Given::UpWithoutSnapshot).await;

        let reply = fx.when_list().await.expect("the list of a running node");

        assert!(
            row_of(&reply, fx.own_id()).is_self,
            "the own row is always in the list"
        );
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_action_waits_for_the_first_snapshot() {
        let fx = setup(Given::UpWithoutSnapshot).await;

        let act = fx.when_forget_self().await.unwrap_err();
        let exit = fx.ctl.find_exit("example.com").unwrap_err();

        let starting = format!("{NET_NAME} is starting");
        assert_eq!(busy_text(&act), starting, "the list action");
        assert_eq!(busy_text_of(&exit), starting, "the domain action");
        fx.finish().await;
    }
}

mod a_node_that_is_up_with_a_snapshot {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_list_is_read_with_a_snapshot() {
        let fx = setup(Given::UpWithSnapshot).await;

        let reply = fx.when_list().await.expect("the list of a running node");

        assert!(row_of(&reply, fx.own_id()).is_self);
        fx.finish().await;
    }
}

enum Given {
    NoNetwork,
    Stopping,
    UpWithoutSnapshot,
    UpWithSnapshot,
}

struct Fixture {
    ctl: Arc<NodeCtl>,
    node: Option<RunningNode>,
}

async fn setup(given: Given) -> Fixture {
    match given {
        Given::NoNetwork => Fixture {
            ctl: NodeCtl::new_without_network(&Settings::default()),
            node: None,
        },
        Given::Stopping => {
            let ctl = NodeCtl::new(&Settings::default());
            ctl.request_stop();
            Fixture { ctl, node: None }
        }
        Given::UpWithoutSnapshot => {
            let fx = attached_node().await;
            fx.drop_the_snapshot();
            fx
        }
        Given::UpWithSnapshot => {
            let fx = attached_node().await;
            fx.wait_until_actions_are_served().await;
            fx
        }
    }
}

/// A loopback node attached to a fresh control; the background task may or may not have collected yet.
async fn attached_node() -> Fixture {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let ctl = NodeCtl::new(&Settings::default());
    ctl.attach(&node);
    Fixture {
        ctl,
        node: Some(node),
    }
}

impl Fixture {
    fn own_id(&self) -> PeerId {
        self.node.as_ref().expect("a fixture with a node").id
    }

    /// Makes the first snapshot "not collected yet": the generation grows, so the running collection
    /// cannot write the cache any more, and the cache is cleared.
    fn drop_the_snapshot(&self) {
        let mut st = self
            .ctl
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.snapshot_gen += 1;
        st.cache = None;
    }

    async fn wait_until_actions_are_served(&self) {
        for _ in 0..200 {
            if self.ctl.running_node_for_action().is_ok() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("the control did not start serving actions in 10 s");
    }

    async fn when_list(&self) -> Result<MembersReply, MemberError> {
        self.ctl.members().await
    }

    async fn when_leave(&self) -> Result<MembersReply, MemberError> {
        self.ctl.member_action(MemberActionReq::Leave).await
    }

    async fn when_forget_self(&self) -> Result<MembersReply, MemberError> {
        let own = self.own_id().to_string();
        self.ctl.member_action(MemberActionReq::Forget(own)).await
    }

    async fn finish(self) {
        if let Some(node) = self.node {
            node.shutdown().await;
        }
    }
}

fn busy_text(e: &MemberError) -> String {
    match e {
        MemberError::Action(ActionError::Busy(m)) => m.clone(),
        other => panic!("not a busy answer: {other:?}"),
    }
}

fn busy_text_of(e: &ActionError) -> String {
    match e {
        ActionError::Busy(m) => m.clone(),
        other => panic!("not a busy answer: {other:?}"),
    }
}

fn row_of(reply: &MembersReply, id: PeerId) -> &MemberRow {
    let full = id.to_string();
    reply
        .members
        .iter()
        .find(|r| r.id_full == full)
        .unwrap_or_else(|| panic!("no row for {full}"))
}
