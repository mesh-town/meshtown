//! Tests of adding slots to a running map: the stop flag, the limit, the end marker and the context.

use std::time::Duration;

use super::super::SlotContext;
use super::*;
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::Relaunch;
use crate::web_gui::{HostError, SlotHandle};

fn handle_of(id: &str) -> (SlotHandle, Arc<NodeCtl>) {
    let ctl = NodeCtl::new(&Settings::default());
    let handle = SlotHandle {
        id: NetSlotId::parse(id).unwrap(),
        node: ctl.clone(),
        config: Arc::new(std::sync::Mutex::new(ConfigState::from_run(&run_config()))),
    };
    (handle, ctl)
}

fn push_idle(slots: &HostSlots, id: &str) -> Result<(), HostError> {
    let (handle, ctl) = handle_of(id);
    slots.push_slot(handle, ctl, async { Ok(()) })
}

fn id_of(i: usize) -> String {
    format!("ab2c{}d", ["a", "b", "c", "d", "e", "f", "g", "h"][i - 2])
}

#[tokio::test]
async fn no_slot_is_added_after_the_stop() {
    let (slots, _ctl) = map();
    slots.stop_all();
    let refused = push_idle(&slots, "ab2c3d");
    assert!(matches!(refused, Err(HostError::Busy(_))), "{refused:?}");
    assert_eq!(slots.len(), 1);
}

#[tokio::test]
async fn no_slot_is_added_after_the_signal_stop() {
    let (slots, _ctl) = map();
    slots.stop_all_on_signal();
    assert!(matches!(
        push_idle(&slots, "ab2c3d"),
        Err(HostError::Busy(_))
    ));
    assert_eq!(slots.len(), 1);
}

#[tokio::test]
async fn the_map_is_full_at_the_limit() {
    let (slots, _ctl) = map();
    for i in 2..=super::super::MAX_NETS {
        push_idle(&slots, &id_of(i)).unwrap();
    }
    assert_eq!(slots.len(), super::super::MAX_NETS);
    let refused = push_idle(&slots, "zz2222");
    assert_eq!(refused, Err(HostError::TooManyNets));
}

#[tokio::test]
async fn broken_slots_take_no_room() {
    let (slots, _ctl) = map();
    slots.mark_broken(NetSlotId::parse("zz2222").unwrap());
    slots.mark_broken(NetSlotId::parse("zz2222").unwrap());
    for i in 2..=super::super::MAX_NETS {
        push_idle(&slots, &id_of(i)).unwrap();
    }
    assert_eq!(slots.broken(), [NetSlotId::parse("zz2222").unwrap()]);
    assert_eq!(slots.len(), super::super::MAX_NETS);
}

#[tokio::test]
async fn a_pushed_slot_ends_with_its_cycle() {
    let (slots, _ctl) = map();
    let (handle, ctl) = handle_of("ab2c3d");
    let waiting = ctl.clone();
    slots
        .push_slot(handle, ctl, async move {
            waiting.next_command().await;
            Ok(())
        })
        .unwrap();
    let mut ended = slots.read()[1].ended.clone().expect("an end marker");
    assert!(!*ended.borrow());
    slots.stop_all();
    tokio::time::timeout(Duration::from_secs(5), ended.wait_for(|e| *e))
        .await
        .expect("the end is reported")
        .unwrap();
}

#[tokio::test]
async fn a_slot_is_removed_but_the_main_one_stays() {
    let (slots, _ctl) = map();
    push_idle(&slots, "ab2c3d").unwrap();
    assert!(slots.remove_slot(&NetSlotId::main()).is_none());
    assert!(slots
        .remove_slot(&NetSlotId::parse("ab2c3d").unwrap())
        .is_some());
    assert_eq!(slots.len(), 1);
}

#[tokio::test]
async fn broken_slot_is_taken_once() {
    let (slots, _ctl) = map();
    let id = NetSlotId::parse("zz2222").unwrap();
    slots.mark_broken(id.clone());
    assert!(slots.take_broken(&id));
    assert!(!slots.take_broken(&id));
    assert!(slots.broken().is_empty());
}

#[tokio::test]
async fn there_is_no_context_until_it_is_set_and_the_first_one_stays() {
    let (slots, _ctl) = map();
    assert!(slots.context().is_none());
    let ctx = || SlotContext {
        main_run: run_config(),
        relaunch: Relaunch {
            file: None,
            cli: CliOverrides::default(),
            secret: run_config().secret,
            initial_domains: Vec::new(),
            blocked_domains: Vec::new(),
            test_net: None,
        },
        sources: GuardSources::default(),
    };
    slots.set_context(ctx());
    slots.set_context(ctx());
    assert!(slots.context().is_some());
}
