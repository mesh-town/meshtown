//! Tests of the slot map: looking slots up, stopping them, the signal stop without the page-request line.

use std::sync::Arc;

use super::HostSlots;
use crate::config::config_file::{
    self, CliOverrides, ConfigSource, NetSlotId, RunConfig, Settings,
};
use crate::node::node_ctl::{Cmd, NodeCtl};
use crate::node::output::{log_page, log_summary};
use crate::web_gui::{ConfigState, HostControl};

fn run_config() -> RunConfig {
    config_file::prepare(&ConfigSource::Disabled, &CliOverrides::default(), false, 0).unwrap()
}

fn map() -> (Arc<HostSlots>, Arc<NodeCtl>) {
    let ctl = NodeCtl::new(&Settings::default());
    let slots = HostSlots::with_main(ctl.clone(), ConfigState::from_run(&run_config()));
    (slots, ctl)
}

fn lines_after(seq: u64) -> String {
    let texts: Vec<String> = log_page(seq, 5000)
        .lines
        .into_iter()
        .map(|l| l.text)
        .collect();
    texts.join("\n")
}

#[tokio::test]
async fn main_slot_is_found_by_its_id_and_others_are_not() {
    let (slots, ctl) = map();
    let main = slots.slot(&NetSlotId::main()).expect("the main slot");
    assert_eq!(main.id, NetSlotId::main());
    let same = slots.main_slot();
    assert!(
        Arc::ptr_eq(&main.config, &same.config),
        "the same config state"
    );
    assert_eq!(
        Arc::as_ptr(&main.node) as *const (),
        Arc::as_ptr(&ctl) as *const (),
        "the same controller"
    );
    assert!(slots.slot(&NetSlotId::parse("ab2c3d").unwrap()).is_none());
}

#[tokio::test]
async fn stop_all_reaches_the_node() {
    let (slots, ctl) = map();
    slots.stop_all();
    assert!(matches!(ctl.next_command().await, Cmd::Stop));
}

#[tokio::test]
async fn stop_on_signal_stops_without_the_page_request_line() {
    let (slots, ctl) = map();
    let before = log_summary(0).last_seq;
    slots.stop_all_on_signal();
    assert!(matches!(ctl.next_command().await, Cmd::Stop));
    assert!(!lines_after(before).contains("at the page's request"));
}

#[tokio::test]
async fn task_is_taken_once_in_slot_order() {
    let (slots, _ctl) = map();
    slots.spawn_task(&NetSlotId::main(), async { Ok(()) });
    let tasks = slots.take_tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].0, NetSlotId::main());
    assert!(slots.take_tasks().is_empty());
}

#[tokio::test]
async fn slots_come_in_the_order_of_the_map() {
    let (slots, _ctl) = map();
    for id in ["zz2222", "ab2c3d"] {
        let ctl = NodeCtl::new(&Settings::default());
        let handle = crate::web_gui::SlotHandle {
            id: NetSlotId::parse(id).unwrap(),
            node: ctl.clone(),
            config: Arc::new(std::sync::Mutex::new(ConfigState::from_run(&run_config()))),
        };
        slots.push_slot(handle, ctl, async { Ok(()) }).unwrap();
    }
    let ids: Vec<String> = slots.slots().iter().map(|s| s.id.to_string()).collect();
    assert_eq!(ids, ["main", "zz2222", "ab2c3d"]);
}

#[path = "saved_slots_tests.rs"]
mod saved_slots;

#[path = "slots_late_tests.rs"]
mod late;
