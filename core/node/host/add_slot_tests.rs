//! Adding a network at run time: a new slot, handing back an empty one, the limit, the shared settings
//! and the domains a new network starts with.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::*;
use crate::config::config_file::{
    self, net_slot_path, CliOverrides, ConfigSource, NetSettings, PathRule, Settings,
};
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::NodeCtl;
use crate::web_gui::test_host::SnapNode;
use crate::web_gui::{ConfigState, DomainInfo, NodeSnapshot};

struct Fixture {
    dir: PathBuf,
    slots: Arc<HostSlots>,
    main: RunConfig,
}

fn fixture(main_ctl: Arc<NodeCtl>) -> Fixture {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("mesh-add-slot-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let main = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let slots = HostSlots::with_main(main_ctl, ConfigState::from_run(&main));
    slots.set_context(SlotContext {
        main_run: main.clone(),
        relaunch: Relaunch {
            file: main.file.clone(),
            cli: CliOverrides::default(),
            secret: main.secret.clone(),
            initial_domains: Vec::new(),
            blocked_domains: Vec::new(),
            test_net: Some(crate::transport::net::mem::process_net()),
        },
        sources: GuardSources::default(),
    });
    Fixture { dir, slots, main }
}

/// A map whose main slot is starting, so it is not an empty slot.
fn busy_map() -> Fixture {
    fixture(NodeCtl::new(&Settings::default()))
}

fn push_busy(slots: &HostSlots, id: &str) {
    let ctl = NodeCtl::new(&Settings::default());
    let handle = SlotHandle {
        id: NetSlotId::parse(id).unwrap(),
        node: ctl.clone(),
        config: Arc::new(Mutex::new(ConfigState::from_run(&placeholder_run()))),
    };
    slots.push_slot(handle, ctl, async { Ok(()) }).unwrap();
}

fn placeholder_run() -> RunConfig {
    config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap()
}

fn id_of(i: usize) -> String {
    format!("ab2c{}d", ["a", "b", "c", "d", "e", "f", "g", "h"][i - 2])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_slot_has_its_own_secret_no_file_and_waits_for_a_key() {
    let fx = busy_map();
    let added = fx.slots.add_slot().unwrap();
    assert!(added.fresh);
    let handle = fx.slots.slot(&added.id).expect("the slot is in the map");
    let secret = handle.config.lock().unwrap().secret.to_bytes();
    assert_ne!(secret, fx.main.secret.to_bytes());
    let path = net_slot_path(&fx.dir.join(config_file::FILE_NAME), &added.id).unwrap();
    assert!(!path.exists(), "the file appears with the first key");
    assert_eq!(handle.node.snapshot().node.status, NodeStatus::Offline);
    fx.slots.stop_all();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adding_again_hands_back_the_empty_slot() {
    let fx = busy_map();
    let first = fx.slots.add_slot().unwrap();
    let again = fx.slots.add_slot().unwrap();
    assert_eq!(again.id, first.id);
    assert!(!again.fresh);
    assert_eq!(fx.slots.len(), 2);
    fx.slots.stop_all();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_main_slot_without_a_key_is_handed_back() {
    let fx = fixture(NodeCtl::new_without_network(&Settings::default()));
    push_busy(&fx.slots, "ab2c3d");
    let added = fx.slots.add_slot().unwrap();
    assert_eq!(added.id, NetSlotId::main());
    assert!(!added.fresh);
    assert_eq!(fx.slots.len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_limit_refuses_a_new_slot() {
    let fx = busy_map();
    (2..=MAX_NETS).for_each(|i| push_busy(&fx.slots, &id_of(i)));
    assert_eq!(fx.slots.add_slot(), Err(HostError::TooManyNets));
    assert_eq!(fx.slots.len(), MAX_NETS);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_slots_take_no_room() {
    let fx = busy_map();
    (2..=7).for_each(|i| push_busy(&fx.slots, &id_of(i)));
    fx.slots.mark_broken(NetSlotId::parse("zz2222").unwrap());
    let added = fx.slots.add_slot().unwrap();
    assert!(added.fresh);
    assert_eq!(fx.slots.len(), 8);
    fx.slots.stop_all();
}

#[tokio::test]
async fn without_a_context_nothing_is_added() {
    let ctl = NodeCtl::new(&Settings::default());
    let slots = HostSlots::with_main(ctl, ConfigState::from_run(&placeholder_run()));
    assert!(matches!(slots.add_slot(), Err(HostError::Busy(_))));
    assert_eq!(slots.len(), 1);
}

#[tokio::test]
async fn nothing_is_added_after_the_stop() {
    let fx = busy_map();
    fx.slots.stop_all();
    assert!(matches!(fx.slots.add_slot(), Err(HostError::Busy(_))));
    assert_eq!(fx.slots.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_settings_follow_the_main_file_and_the_others_do_not() {
    let fx = busy_map();
    {
        let main = fx.slots.main_slot();
        let mut state = main.config.lock().unwrap();
        let net = &mut state.file_settings.net;
        net.probe_interval_sec = 120;
        net.checks_enabled = false;
        net.exit_enabled = !NetSettings::default().exit_enabled;
        net.node_name = Some("laptop".into());
    }
    let added = fx.slots.add_slot().unwrap();
    let handle = fx.slots.slot(&added.id).unwrap();
    let state = handle.config.lock().unwrap();
    let net = &state.file_settings.net;
    assert_eq!(net.probe_interval_sec, 120);
    assert!(!net.checks_enabled);
    assert_eq!(net.exit_enabled, NetSettings::default().exit_enabled);
    assert_eq!(net.node_name, None);
    drop(state);
    fx.slots.stop_all();
}

#[tokio::test]
async fn a_new_network_starts_with_the_domains_of_the_running_ones() {
    let fx = busy_map();
    for (id, name) in [
        ("ab2c3d", "a.org"),
        ("ab2c4d", "b.org"),
        ("ab2c5d", "a.org"),
    ] {
        let snapshot = NodeSnapshot {
            domains: vec![DomainInfo {
                name: name.into(),
                ..DomainInfo::default()
            }],
            ..NodeSnapshot::default()
        };
        let handle = SlotHandle {
            id: NetSlotId::parse(id).unwrap(),
            node: SnapNode::new(snapshot),
            config: Arc::new(Mutex::new(ConfigState::from_run(&placeholder_run()))),
        };
        let ctl = NodeCtl::new(&Settings::default());
        fx.slots.push_slot(handle, ctl, async { Ok(()) }).unwrap();
    }
    let ctx = fx.slots.context().unwrap();
    let relaunch = fx.slots.relaunch_for_new(ctx, &fx.main);
    assert_eq!(relaunch.initial_domains, ["a.org", "b.org"]);
}

mod slot_with_an_invite_join {
    use super::*;
    use crate::web_gui::{InviteJoinState, InviteJoinView, NodeRuntime};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn waiting_for_members_is_not_handed_back_a_new_slot_is_made() {
        // given
        let fx = busy_map();
        push_offline_slot(&fx.slots, "ab2c3d", Some(InviteJoinState::Waiting));

        // when
        let added = fx.slots.add_slot().unwrap();

        // then
        assert!(added.fresh, "the waiting slot must stay untouched");
        assert_ne!(added.id, NetSlotId::parse("ab2c3d").unwrap());
        assert_eq!(fx.slots.len(), 3);
        fx.slots.stop_all();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn refused_is_handed_back_as_an_empty_slot() {
        // given
        let fx = busy_map();
        push_offline_slot(&fx.slots, "ab2c3d", Some(InviteJoinState::Refused));

        // when
        let added = fx.slots.add_slot().unwrap();

        // then
        assert_eq!(added.id, NetSlotId::parse("ab2c3d").unwrap());
        assert!(!added.fresh);
        assert_eq!(fx.slots.len(), 2);
        fx.slots.stop_all();
    }

    /// An added slot without a key whose node is offline, with the given invite-join state, if any.
    fn push_offline_slot(slots: &HostSlots, id: &str, invite: Option<InviteJoinState>) {
        let snapshot = NodeSnapshot {
            node: NodeRuntime {
                status: NodeStatus::Offline,
                ..NodeRuntime::default()
            },
            ..NodeSnapshot::default()
        };
        let node = SnapNode::new(snapshot);
        *node.invite_join.lock().unwrap() = invite.map(|state| InviteJoinView {
            state,
            message: "text".to_string(),
            label: None,
            expires_unix: 1_800_000_000,
        });
        let handle = SlotHandle {
            id: NetSlotId::parse(id).unwrap(),
            node,
            config: Arc::new(Mutex::new(ConfigState::from_run(&placeholder_run()))),
        };
        let ctl = NodeCtl::new(&Settings::default());
        slots.push_slot(handle, ctl, async { Ok(()) }).unwrap();
    }
}

/// A join started in the same turn as `add_slot`, before the slot task has run, still knows its owner key.
#[tokio::test]
async fn join_right_after_add_slot_has_owner() {
    use crate::invite::{InviteHead, InviteSecret};
    use crate::web_gui::InviteJoinState;

    let fx = busy_map();
    let added = fx.slots.add_slot().unwrap();
    let slot = fx.slots.slot(&added.id).expect("the slot is in the map");
    let made_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let head = InviteHead {
        secret: InviteSecret::generate().unwrap(),
        network_id: [1, 2, 3, 4],
        label: None,
        made_unix,
        root: crate::transport::net::contract::pid(7),
    };
    slot.node
        .join_by_invite(head, vec![], Box::new(|_, _| Ok(())))
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let view = slot.node.invite_join().expect("the wait is on");
    assert_eq!(view.state, InviteJoinState::Waiting, "{}", view.message);
    fx.slots.stop_all();
}
