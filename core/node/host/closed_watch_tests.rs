//! Tests of the slot watcher for a deleted network (`closed_watch.rs`): a new closing in the slot's channel
//! makes the slot leave the way a manual Leave does and notes the deletion exactly once.
//! Nodes: the closing administrator is the node with seed 2.

// assumed: this file is declared in `closed_watch.rs` as
// `#[cfg(test)] #[path = "closed_watch_tests.rs"] mod tests;`.
// assumed: the single hook of the deletion note is `HostSlots.deleted.list() -> Vec<DeletedNet>`
// (`cfg(test)`); `DeletedNet { label, id_hex, by, by_name, at_unix }`.

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::config::config_file::{self, CliOverrides, ConfigSource, PathRule, RunConfig, Settings};
use crate::node::host::retire::LEAVE_GRACE;
use crate::node::node_ctl::NodeCtl;
use crate::transport::net::contract::pid;
use crate::web_gui::{ConfigState, SlotHandle};

const KEY: &str = "alpha--abacus-abdomen-0123456789abcdefghjkm";

mod added_slot_with_a_closed_network {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn leaves_the_map_and_retires_once() {
        // given: an added slot that runs a node cycle
        let fx = setup(Opts::default());
        fx.add_waiting("ab2c3d");
        // when: the channel carries a closing
        fx.when_closed(Some(closing(5))).await;
        // then: the slot is gone and one leave task is registered
        assert!(fx.slots.slot(&id_of("ab2c3d")).is_none());
        assert_eq!(fx.slots.take_retiring().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn notes_the_deletion_once_with_the_network_and_the_admin() {
        let fx = setup(Opts::default());
        fx.add_waiting("ab2c3d");
        fx.when_closed(Some(closing(5))).await;
        let notes = fx.notes();
        assert_eq!(notes.len(), 1, "one note for one deletion: {notes:?}");
        assert_eq!(notes[0].label.as_deref(), Some("alpha"));
        assert!(notes[0].id_hex.is_some(), "the network id is noted");
        assert_eq!(notes[0].by, pid(2));
        assert_eq!(notes[0].at_unix, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn the_stop_waits_for_the_grace_because_the_leave_is_announced() {
        // given / when
        let fx = setup(Opts::default());
        let ctl = fx.add_waiting("ab2c3d");
        fx.when_closed(Some(closing(5))).await;
        // then: the node is not stopped at once, but after the grace
        assert!(
            !ctl.stop_requested(),
            "the record must reach the neighbours first"
        );
        tokio::time::sleep(LEAVE_GRACE + Duration::from_millis(200)).await;
        assert!(ctl.stop_requested(), "the node stops after the grace");
    }

    #[tokio::test(start_paused = true)]
    async fn the_same_value_again_gives_no_second_note() {
        // given: the slot has left and noted the deletion
        let fx = setup(Opts::default());
        fx.add_waiting("ab2c3d");
        fx.when_closed(Some(closing(5))).await;
        // when: the same closing is written again, and again after a gap
        fx.when_closed(Some(closing(5))).await;
        fx.when_closed(None).await;
        fx.when_closed(Some(closing(5))).await;
        // then
        assert_eq!(fx.notes().len(), 1);
        assert_eq!(fx.slots.take_retiring().len(), 1);
    }
}

mod added_slot_without_a_closing {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn stays_alive_when_the_channel_carries_none() {
        // given: a slot whose network got a close of a non-administrator, which the count ignores
        let fx = setup(Opts::default());
        fx.add_waiting("ab2c3d");
        // when: the channel stays none
        fx.when_closed(None).await;
        // then: the slot lives and nothing is noted or retired
        assert!(fx.slots.slot(&id_of("ab2c3d")).is_some());
        assert!(fx.notes().is_empty());
        assert!(fx.slots.take_retiring().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_manual_leave_first_gives_no_note() {
        // given: the slot was left by hand
        let fx = setup(Opts::default());
        fx.add_waiting("ab2c3d");
        fx.slots.retire(&id_of("ab2c3d"), false).unwrap();
        // when: the closing arrives afterwards
        fx.when_closed(Some(closing(5))).await;
        // then: the slot is gone for good and the deletion is not noted
        assert!(fx.notes().is_empty(), "{:?}", fx.notes());
        assert_eq!(fx.slots.take_retiring().len(), 1, "only the manual leave");
    }

    #[tokio::test(start_paused = true)]
    async fn a_stopping_host_gives_no_note_and_keeps_the_slot() {
        // given: the host is stopping while the slot's cycle still runs
        let fx = setup(Opts::default());
        fx.add_pending("ab2c3d");
        fx.slots.stop_all_on_signal();
        // when
        fx.when_closed(Some(closing(5))).await;
        // then
        assert!(fx.slots.slot(&id_of("ab2c3d")).is_some());
        assert!(fx.notes().is_empty());
        assert!(fx.slots.take_retiring().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn the_watcher_ends_with_the_cycle_of_the_slot() {
        // given: the cycle of the slot has ended
        let fx = setup(Opts::default());
        let end = fx.add_ending("ab2c3d");
        end.send(()).unwrap();
        settle().await;
        // when: a closing arrives afterwards
        fx.when_closed(Some(closing(5))).await;
        // then: nobody reacts
        assert!(fx.slots.slot(&id_of("ab2c3d")).is_some());
        assert!(fx.notes().is_empty());
        assert!(fx.slots.take_retiring().is_empty());
    }
}

mod main_slot_with_a_closed_network {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn leaves_in_place_and_notes_once() {
        // given: the main slot with a key from the settings file
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        // when
        fx.when_closed_main(Some(closing(5))).await;
        // then: the slot stays, a leave task runs, one note is made
        assert!(fx.slots.slot(&NetSlotId::main()).is_some());
        assert_eq!(fx.slots.take_retiring().len(), 1);
        assert_eq!(fx.notes().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn the_same_value_and_a_return_to_it_give_no_second_note() {
        // given: the first closing was handled
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        fx.when_closed_main(Some(closing(5))).await;
        // when: the same value again, then none and the same value once more
        fx.when_closed_main(Some(closing(5))).await;
        fx.when_closed_main(None).await;
        fx.when_closed_main(Some(closing(5))).await;
        // then: still one note and no second leave task
        assert_eq!(fx.notes().len(), 1);
        assert_eq!(fx.slots.take_retiring().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_closing_after_a_switch_fires_again() {
        // given: the closing of the first network was handled
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        fx.when_closed_main(Some(closing(5))).await;
        // when: the slot moves to another network (none in between) and that one is closed
        fx.when_closed_main(None).await;
        fx.when_closed_main(Some(closing(9))).await;
        // then: two deletions are noted
        let notes = fx.notes();
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert_eq!(notes[1].at_unix, 9);
    }

    #[tokio::test(start_paused = true)]
    async fn a_key_from_a_flag_stays_and_the_deletion_is_noted() {
        // given: the key comes from --key, so the network cannot be left
        let fx = setup(Opts {
            main_key: true,
            key_from_flag: true,
        });
        fx.spawn_main_cycle();
        // when
        fx.when_closed_main(Some(closing(5))).await;
        // then: no leave task, the key is kept, one note
        assert!(fx.slots.take_retiring().is_empty());
        assert_eq!(fx.main_key(), Some(KEY.to_string()));
        assert_eq!(fx.notes().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_manual_leave_twice_and_a_closing_start_one_leave() {
        // given: the main network is being left by hand, the button pressed twice
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        fx.slots.retire(&NetSlotId::main(), false).unwrap();
        fx.slots.retire(&NetSlotId::main(), false).unwrap();
        // when: the closing arrives while the leave goes on
        fx.when_closed_main(Some(closing(5))).await;
        // then: only the first leave started a task
        assert_eq!(fx.slots.take_retiring().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_closing_and_then_a_manual_leave_start_one_leave() {
        // given: the closing started the leave
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        fx.when_closed_main(Some(closing(5))).await;
        // when: the button is pressed while it goes on
        fx.slots.retire(&NetSlotId::main(), false).unwrap();
        // then: no second task
        assert_eq!(fx.slots.take_retiring().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn spawning_the_cycle_again_replaces_the_watcher() {
        // given: the cycle of the main slot was spawned twice
        let fx = setup(Opts {
            main_key: true,
            ..Opts::default()
        });
        fx.spawn_main_cycle();
        fx.spawn_main_cycle();
        // when
        fx.when_closed_main(Some(closing(5))).await;
        // then: one watcher acted, so one note
        assert_eq!(fx.notes().len(), 1, "{:?}", fx.notes());
    }
}

// --- setup and helpers ---

#[derive(Default)]
struct Opts {
    /// The main slot holds a network key in its settings file.
    main_key: bool,
    /// The key came from a flag, so the network cannot be left.
    key_from_flag: bool,
}

struct Fixture {
    _dir: PathBuf,
    slots: Arc<HostSlots>,
    main_ctl: Arc<NodeCtl>,
    /// The controller of the slot added last.
    added: Mutex<Option<Arc<NodeCtl>>>,
}

fn closing(at_unix: u64) -> Closed {
    Closed {
        by: pid(2),
        at_unix,
    }
}

fn id_of(text: &str) -> NetSlotId {
    NetSlotId::parse(text).unwrap()
}

fn placeholder_run() -> RunConfig {
    config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap()
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-closed-watch-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Lets the watcher tasks run.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(10)).await;
}

/// A map whose main slot has a settings file, with a network key when the options say so.
fn setup(opts: Opts) -> Fixture {
    let dir = temp_dir();
    let path = dir.join(config_file::FILE_NAME);
    let source = ConfigSource::File {
        path: path.clone(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let mut state = ConfigState::from_run(&run);
    if opts.main_key {
        let mut s = state.file_settings.clone();
        s.net.network_key = Some(KEY.to_string());
        let rev =
            config_file::save_config_file(&path, Some(state.file_report.rev), 1, &state.secret, &s)
                .unwrap();
        state.file_settings = s;
        state.file_report.rev = rev;
        state.current_network_key = Some(KEY.to_string());
    }
    if opts.key_from_flag {
        state.cli_override_paths = vec!["network.key"];
    }
    let main_ctl = NodeCtl::new(&Settings::default());
    let slots = HostSlots::with_main(main_ctl.clone(), state);
    Fixture {
        _dir: dir,
        slots,
        main_ctl,
        added: Mutex::new(None),
    }
}

impl Fixture {
    /// Sets the closing in the channel of the added slot and lets the watchers react.
    async fn when_closed(&self, value: Option<Closed>) {
        let ctl = self.added.lock().unwrap().clone();
        ctl.expect("a slot was added").set_closed_for_test(value);
        settle().await;
    }

    async fn when_closed_main(&self, value: Option<Closed>) {
        self.main_ctl.set_closed_for_test(value);
        settle().await;
    }

    fn notes(&self) -> Vec<DeletedNet> {
        self.slots.deleted.list()
    }

    fn main_key(&self) -> Option<String> {
        let main = self.slots.main_slot();
        let state = main.config.lock().unwrap();
        state.current_network_key.clone()
    }

    /// The main slot gets a node cycle that never ends.
    fn spawn_main_cycle(&self) {
        self.slots
            .spawn_task(&NetSlotId::main(), std::future::pending());
    }

    /// An added slot whose cycle ends when the node is told to stop.
    fn add_waiting(&self, id: &str) -> Arc<NodeCtl> {
        self.add_slot(id, |ctl| async move {
            ctl.next_command().await;
            Ok(())
        })
    }

    /// An added slot whose cycle never ends by itself.
    fn add_pending(&self, id: &str) -> Arc<NodeCtl> {
        self.add_slot(id, |_| std::future::pending())
    }

    /// An added slot whose cycle ends when the returned sender fires.
    fn add_ending(&self, id: &str) -> tokio::sync::oneshot::Sender<()> {
        let (end, ended) = tokio::sync::oneshot::channel::<()>();
        self.add_slot(id, |_| async move {
            let _ = ended.await;
            Ok(())
        });
        end
    }

    fn add_slot<F, Fut>(&self, id: &str, cycle: F) -> Arc<NodeCtl>
    where
        F: FnOnce(Arc<NodeCtl>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let ctl = NodeCtl::new(&Settings::default());
        let mut state = ConfigState::from_run(&placeholder_run());
        state.current_network_key = Some(KEY.to_string());
        let handle = SlotHandle {
            id: id_of(id),
            node: ctl.clone(),
            config: Arc::new(Mutex::new(state)),
        };
        let running = cycle(ctl.clone());
        self.slots.push_slot(handle, ctl.clone(), running).unwrap();
        *self.added.lock().unwrap() = Some(ctl.clone());
        ctl
    }
}
