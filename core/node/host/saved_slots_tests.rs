//! Starting the networks saved under `nets/`: which slots come up beside the main one, which are refused
//! and what the log says about them. The network is `mem` only.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::super::HostSlots;
use crate::config::config_file::{
    self, net_slot_path, new_slot_id, save_net_file, CliOverrides, ConfigSource, NetSettings,
    NetSlotId, PathRule, RunConfig,
};
use crate::members::network_key::NetworkKey;
use crate::node::guard_sources::GuardSources;
use crate::node::host::MAX_NETS;
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output::{log_page, log_summary};
use crate::transport::net::NodeSecret;
use crate::web_gui::{ConfigState, HostControl};

const NOW: u64 = 1_790_000_000;
const CREATED: u64 = 1_792_000_000;

mod main_and_two_saved_networks {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn all_three_have_a_handle() {
        // given: the main slot and two slots on disk
        let mut fx = setup(10);
        let first = fx.add_slot(1, CREATED);
        let second = fx.add_slot(2, CREATED + 1);
        // when: the saved slots are started
        fx.when_started();
        // then: the host holds a handle for the main slot and for each saved one
        for id in [NetSlotId::main(), first, second] {
            let handle = fx
                .slots
                .slot(&id)
                .unwrap_or_else(|| panic!("no handle for {id}"));
            assert_eq!(handle.id, id);
        }
        assert_eq!(fx.slots.len(), 3, "main and two saved slots");
        fx.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn both_saved_nodes_come_up() {
        // given: two slots on disk
        let mut fx = setup(30);
        fx.add_slot(1, CREATED);
        fx.add_slot(2, CREATED + 1);
        // when: the saved slots are started
        fx.when_started();
        // then: both nodes are up (the main slot has no node here)
        assert!(fx.wait_nodes_up(2).await, "two saved nodes must come up");
        fx.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn main_file_stays_byte_for_byte() {
        // given: a main file and a saved slot
        let mut fx = setup(50);
        fx.add_slot(1, CREATED);
        let before = std::fs::read(fx.main_path()).expect("main file");
        // when: the saved slots are started
        fx.when_started();
        // then: the main file is not written
        let after = std::fs::read(fx.main_path()).expect("main file");
        assert_eq!(before, after, "starting slots must not write the main file");
        fx.stop().await;
    }
}

mod more_networks_than_the_limit {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn only_the_limit_minus_the_main_slot_is_started() {
        // given: one slot more than the process may run beside the main one
        let mut fx = setup(70);
        let ids = fx.add_slots(MAX_NETS + 1);
        // when: the saved slots are started
        fx.when_started();
        // then: the first ones are in the map, the last two are not
        assert_eq!(
            fx.slots.len(),
            MAX_NETS,
            "the main slot counts toward the limit"
        );
        let (started, refused) = ids.split_at(MAX_NETS - 1);
        for id in started {
            assert!(fx.slots.slot(id).is_some(), "slot {id} must be started");
        }
        for id in refused {
            assert!(fx.slots.slot(id).is_none(), "slot {id} is over the limit");
        }
        fx.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn each_refused_slot_gets_a_limit_line() {
        // given: one slot more than the limit allows
        let mut fx = setup(90);
        let ids = fx.add_slots(MAX_NETS + 1);
        // when: the saved slots are started
        fx.when_started();
        // then: exactly the last two slots have a "limit" line, with the number from the constant
        let (started, refused) = ids.split_at(MAX_NETS - 1);
        for id in refused {
            let line = fx
                .not_started_line(id)
                .unwrap_or_else(|| panic!("no line for {id}"));
            let want = format!("limit of {MAX_NETS} networks");
            assert!(line.contains(&want), "{line}");
        }
        for id in started {
            assert_eq!(fx.not_started_line(id), None, "slot {id} was started");
        }
        fx.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn refused_folders_stay_on_disk() {
        // given: one slot more than the limit allows
        let mut fx = setup(110);
        let ids = fx.add_slots(MAX_NETS + 1);
        // when: the saved slots are started
        fx.when_started();
        // then: the files of the refused slots are still there
        for id in &ids[MAX_NETS - 1..] {
            let path = net_slot_path(&fx.main_path(), id).unwrap();
            assert!(path.is_file(), "the folder of {id} must be kept");
        }
        fx.stop().await;
    }
}

mod copy_of_a_slot_folder {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_second_folder_is_refused_as_the_same_network() {
        // given: a slot and a copy of its folder under another name
        let mut fx = setup(130);
        let original = fx.add_slot(1, CREATED);
        let copy = fx.copy_slot(&original);
        let (first, second) = if original < copy {
            (original, copy)
        } else {
            (copy, original)
        };
        // when: the saved slots are started
        fx.when_started();
        // then: the one that sorts later is refused and names the other one
        let line = fx.not_started_line(&second).expect("a line for the copy");
        assert!(
            line.contains(&format!("same network as slot {first}")),
            "{line}"
        );
        assert_eq!(fx.not_started_line(&first), None);
        fx.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn only_one_node_runs() {
        // given: a slot and a copy of its folder
        let mut fx = setup(150);
        let original = fx.add_slot(1, CREATED);
        fx.copy_slot(&original);
        // when: the saved slots are started
        fx.when_started();
        // then: one saved slot is in the map and one node comes up
        assert_eq!(fx.slots.len(), 2, "main and one saved slot");
        assert!(fx.wait_nodes_up(1).await, "the node must come up");
        fx.stop().await;
    }
}

struct Fixture {
    dir: PathBuf,
    main: RunConfig,
    relaunch: Relaunch,
    slots: Arc<HostSlots>,
    sources: GuardSources,
    seed: u8,
    log_mark: u64,
}

impl Fixture {
    fn main_path(&self) -> PathBuf {
        self.dir.join(config_file::FILE_NAME)
    }

    /// Writes a slot file whose network key is `seed + offset`; offset 0 is the key of the main network.
    fn add_slot(&mut self, offset: u8, created_unix: u64) -> NetSlotId {
        let id = new_slot_id(&self.main_path()).expect("a free slot id");
        let key = NetworkKey::from_bytes([self.seed + offset; 32]);
        let net = NetSettings {
            network_key: Some(key.encode()),
            checks_enabled: false,
            ..NetSettings::default()
        };
        let path = net_slot_path(&self.main_path(), &id).unwrap();
        let secret = NodeSecret::from_bytes(&[self.seed + offset; 32]);
        save_net_file(&path, None, NOW, created_unix, &secret, &net).expect("write the slot file");
        id
    }

    /// `count` slots with distinct networks; the load order is the order of the returned ids.
    fn add_slots(&mut self, count: usize) -> Vec<NetSlotId> {
        (0..count)
            .map(|i| self.add_slot(1 + i as u8, CREATED + i as u64))
            .collect()
    }

    /// Copies the folder of a slot under a new name.
    fn copy_slot(&mut self, from: &NetSlotId) -> NetSlotId {
        let copy = new_slot_id(&self.main_path()).expect("a free slot id");
        let src = net_slot_path(&self.main_path(), from).unwrap();
        let dst = net_slot_path(&self.main_path(), &copy).unwrap();
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(src, dst).expect("copy the slot file");
        copy
    }

    fn when_started(&self) {
        self.slots
            .start_saved_slots(&self.main, &self.relaunch, &self.sources, NOW);
    }

    /// The log line that says the slot was not started; `None` — no such line.
    fn not_started_line(&self, id: &NetSlotId) -> Option<String> {
        let want = format!("net slot {id} not started");
        log_page(self.log_mark, 5000)
            .lines
            .into_iter()
            .map(|l| l.text)
            .find(|t| t.contains(&want))
    }

    /// Waits until `count` nodes have put their handles into the guard sources, i.e. are up.
    async fn wait_nodes_up(&self, count: usize) -> bool {
        for _ in 0..300 {
            if self.sources.snapshot().len() == count {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// Stops every node and waits for the slot tasks.
    async fn stop(&mut self) {
        self.slots.stop_all();
        for (id, task) in self.slots.take_tasks() {
            let done = tokio::time::timeout(Duration::from_secs(20), task).await;
            assert!(done.is_ok(), "the task of slot {id} did not stop");
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// A main file with a network key (`seed`) and a host that holds only the main slot, without a node.
/// Keys of the saved slots are `seed + 1`, `seed + 2`, ... so tests with different seeds never share a network.
fn setup(seed: u8) -> Fixture {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-saved-slots-test-{}-{nanos}-{seed}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let overrides = CliOverrides {
        network_key: Some(NetworkKey::from_bytes([seed; 32]).encode()),
        checks_enabled: Some(false),
        socks_port: Some(0),
        web_port: Some(0),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let main = config_file::prepare(&source, &overrides, true, 0).expect("prepare the main file");
    let relaunch = Relaunch {
        file: main.file.clone(),
        cli: overrides,
        secret: main.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(crate::transport::net::mem::process_net()),
    };
    let ctl = NodeCtl::new(&main.effective_settings);
    let slots = HostSlots::with_main(ctl, ConfigState::from_run(&main));
    Fixture {
        dir,
        main,
        relaunch,
        slots,
        sources: GuardSources::default(),
        seed,
        log_mark: log_summary(0).last_seq,
    }
}

mod network_label_switch {
    use super::*;
    use crate::node::host::saved_slots::{has_saved_slots, multi_net_for};

    #[tokio::test]
    async fn one_network_keeps_the_label_off() {
        // given: only the main slot and no saved folders
        let fx = setup(170);
        // when / then: nothing asks for the label
        assert!(!has_saved_slots(Some(&fx.main)));
        assert!(!multi_net_for(1));
    }

    #[tokio::test]
    async fn an_added_network_turns_the_label_on() {
        // given: the main slot and one saved folder
        let mut fx = setup(180);
        fx.add_slot(1, CREATED);
        // when / then: the folder is seen up front, and two running slots show the label
        assert!(has_saved_slots(Some(&fx.main)));
        assert!(multi_net_for(2));
    }
}

mod without_a_settings_file {
    use super::*;
    use crate::node::host::saved_slots::has_saved_slots;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn no_net_files_are_written_or_read() {
        // given: `--no-config`: no settings file, and no `nets` folder
        let fx = setup(190);
        let overrides = CliOverrides {
            network_key: Some(NetworkKey::from_bytes([190; 32]).encode()),
            ..CliOverrides::default()
        };
        let main = config_file::prepare(&ConfigSource::Disabled, &overrides, true, 0)
            .expect("prepare without a file");
        let slots = HostSlots::with_main(
            NodeCtl::new(&main.effective_settings),
            ConfigState::from_run(&main),
        );
        // when: the saved slots are started
        slots.start_saved_slots(&main, &fx.relaunch, &fx.sources, NOW);
        // then: no file, no extra slot, no `nets` folder anywhere near the test directory
        assert!(main.file.is_none());
        assert!(!has_saved_slots(Some(&main)));
        assert_eq!(slots.len(), 1, "the main slot only");
        assert!(!fx.dir.join("nets").exists());
        assert!(!std::env::current_dir().unwrap().join("nets").exists());
    }
}

#[path = "saved_slots_broken_tests.rs"]
mod broken;
