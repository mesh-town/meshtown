//! `Job::accept` with a new node key: the key goes to `save` and into the switch command together with
//! the network key; a stale accept drops it.

use super::*;
use crate::config::config_file::Settings;
use crate::invite::InviteSecret;
use crate::transport::net::NodeSecret;
use crate::web_gui::NodeControl;
use std::sync::Mutex;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accept_with_a_new_key_saves_and_switches_with_it() {
    // given: a running wait and its generation
    let fx = setup();
    let new = NodeSecret::from_bytes(&[77; 32]);
    // when
    fx.job(fx.current_gen())
        .accept("mesh2-key".to_string(), Some(new.clone()));
    // then: `save` got the new key, the node got it in the same command
    assert_eq!(fx.saved_owners(), vec![Some(new.peer_id())]);
    match fx.ctl.next_command().await {
        Cmd::Switch { key, secret, .. } => {
            assert_eq!(key, "mesh2-key");
            assert_eq!(secret.map(|s| s.peer_id()), Some(new.peer_id()));
        }
        other => panic!("expected a switch with the new key, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_accept_drops_the_new_key() {
    // given: a wait that was replaced by a newer one
    let fx = setup();
    let stale = fx.current_gen();
    fx.ctl
        .join_by_invite(tests_head(), vec![], fx.save())
        .unwrap();
    assert_ne!(fx.current_gen(), stale, "given: the wait was replaced");
    // when: the old wait delivers a key with a new node key
    fx.job(stale).accept(
        "mesh2-key".to_string(),
        Some(NodeSecret::from_bytes(&[77; 32])),
    );
    // then: nothing is saved and no command is sent
    assert_eq!(
        fx.saved_owners(),
        Vec::<Option<crate::transport::net::PeerId>>::new()
    );
    let cmd = tokio::time::timeout(Duration::from_millis(100), fx.ctl.next_command()).await;
    assert!(cmd.is_err(), "a stale accept must send no command: {cmd:?}");
}

struct Fx {
    ctl: Arc<NodeCtl>,
    owners: Arc<Mutex<Vec<Option<crate::transport::net::PeerId>>>>,
}

fn setup() -> Fx {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    ctl.set_owner_secret(NodeSecret::from_bytes(&[5; 32]));
    ctl.set_test_net(Some(crate::transport::net::mem::MemNet::new()));
    let fx = Fx {
        ctl,
        owners: Arc::default(),
    };
    fx.ctl
        .join_by_invite(tests_head(), vec![], fx.save())
        .unwrap();
    fx
}

impl Fx {
    fn save(&self) -> SaveKey {
        let owners = self.owners.clone();
        Box::new(move |_, new| {
            owners.lock().unwrap().push(new.map(|s| s.peer_id()));
            Ok(())
        })
    }

    fn current_gen(&self) -> u64 {
        self.ctl.ctl_shared.ctl_state.lock().unwrap().invite_gen
    }

    fn saved_owners(&self) -> Vec<Option<crate::transport::net::PeerId>> {
        self.owners.lock().unwrap().clone()
    }

    fn job(&self, invite_gen: u64) -> Job {
        Job {
            ctl_shared: self.ctl.ctl_shared.clone(),
            cmd_tx: self.ctl.cmd_tx.clone(),
            invite_gen,
            head: tests_head(),
            peers: vec![],
            save: self.save(),
            test_net: None,
            owner: None,
        }
    }
}

fn tests_head() -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([4; 32]),
        network_id: [1, 2, 3, 4],
        label: None,
        made_unix: now_unix(),
        root: crate::transport::net::contract::pid(7),
    }
}
