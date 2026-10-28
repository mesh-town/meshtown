//! The demo's network slots: `--nets N` starts N fake networks, `add_slot` adds an empty one.

use std::sync::{Arc, Mutex};

use meshtown_core::config::config_file::{NetSlotId, RunConfig};
use meshtown_core::members::network_key::format_managed_key;
use meshtown_core::members::words;
use meshtown_core::web_gui::{AddedSlot, ConfigState, HostControl, HostError, SlotHandle};

use clap::Parser;

use crate::args::Args;
use crate::background::run_background;
use crate::managed::demo_root;
use crate::node::DemoNode;

/// Most networks one page shows.
const MAX_NETS: usize = 8;
/// Slot ids are `demo` plus two letters; the second letter is the creation counter.
const MAX_CREATED: u8 = 26;

struct Entry {
    handle: SlotHandle,
    task: Option<tokio::task::JoinHandle<()>>,
}

struct Slots {
    entries: Vec<Entry>,
    created: u8,
}

pub(crate) struct DemoHost {
    slots: Mutex<Slots>,
    template: RunConfig,
}

fn slot_id(n: u8) -> NetSlotId {
    let text = format!("demoa{}", (b'a' + n) as char);
    NetSlotId::parse(&text).expect("demo slot id is valid")
}

/// The run of network number `index` (2 and up): its own key label, the rest as in `run`.
fn run_for_net(run: &RunConfig, index: u8, with_network: bool) -> RunConfig {
    let mut out = run.clone();
    let key = with_network
        .then(|| words::generate_tail().ok())
        .flatten()
        .map(|tail| {
            format_managed_key(Some(&format!("demo-network-{index}")), &tail, &demo_root())
        });
    out.effective_settings.net.network_key = key.clone();
    out.file_settings.net.network_key = key;
    out
}

fn spawn_background(demo: &Arc<DemoNode>) -> tokio::task::JoinHandle<()> {
    meshtown_core::node::output::spawn(run_background(demo.clone()))
}

impl DemoHost {
    /// The main slot wraps `main_node`; with `--nets N` the slots 2..=N follow, each with its own
    /// node and network key. Its background tasks start here, so it must run inside a runtime.
    pub(crate) fn new(args: &Args, run: &RunConfig, main_node: Arc<DemoNode>) -> Arc<Self> {
        let key = main_node
            .state
            .lock()
            .unwrap()
            .settings
            .net
            .network_key
            .clone();
        if let Some(key) = key {
            crate::invites::remember(&key);
        }
        let main = Entry {
            handle: SlotHandle {
                id: NetSlotId::main(),
                node: main_node,
                config: Arc::new(Mutex::new(ConfigState::from_run(run))),
            },
            task: None,
        };
        let host = Arc::new(DemoHost {
            slots: Mutex::new(Slots {
                entries: vec![main],
                created: 0,
            }),
            template: run.clone(),
        });
        for index in 2..=args.nets {
            host.start_net(args, index);
        }
        host
    }

    fn start_net(&self, args: &Args, index: u8) {
        let run = run_for_net(&self.template, index, !args.no_network);
        let mut sub = clone_args(args);
        sub.seed = args.seed.max(1) + u64::from(index) - 1;
        let node = Arc::new(crate::build_demo_node(&sub, &run));
        let config = ConfigState::from_run(&run);
        self.push(node, config);
    }

    fn push(&self, node: Arc<DemoNode>, config: ConfigState) -> NetSlotId {
        let key = node.state.lock().unwrap().settings.net.network_key.clone();
        if let Some(key) = key {
            crate::invites::remember(&key);
        }
        let mut slots = self.slots.lock().unwrap();
        let id = slot_id(slots.created);
        slots.created += 1;
        let task = Some(spawn_background(&node));
        let handle = SlotHandle {
            id: id.clone(),
            node,
            config: Arc::new(Mutex::new(config)),
        };
        slots.entries.push(Entry { handle, task });
        id
    }
}

/// The per-node flags of `a`, as a fresh `Args` the extra slots can adjust.
fn clone_args(a: &Args) -> Args {
    let mut flags = vec!["web_demo".to_string()];
    flags.push(format!("--seed={}", a.seed));
    flags.push(format!("--fast={}", a.fast));
    flags.push(format!("--offline-peers={}", a.offline_peers));
    flags.push(format!("--no-exit-peers={}", a.no_exit_peers));
    flags.push(format!("--dht-nodes={}", a.dht_nodes));
    let toggles = [
        (a.no_network, "--no-network"),
        (a.dht_joined, "--dht-joined"),
        (a.me_excluded, "--me-excluded"),
        (a.members_pending, "--members-pending"),
        (a.admission_refused, "--admission-refused"),
        (a.second_admin, "--second-admin"),
    ];
    flags.extend(toggles.iter().filter(|t| t.0).map(|t| t.1.to_string()));
    if let Some(n) = a.ring_lines {
        flags.push(format!("--ring-lines={n}"));
    }
    Args::parse_from(flags)
}

impl HostControl for DemoHost {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        self.slots().into_iter().find(|s| &s.id == id)
    }

    fn main_slot(&self) -> SlotHandle {
        self.slots.lock().unwrap().entries[0].handle.clone()
    }

    fn slots(&self) -> Vec<SlotHandle> {
        let slots = self.slots.lock().unwrap();
        slots.entries.iter().map(|e| e.handle.clone()).collect()
    }

    fn stop_all(&self) {
        for slot in self.slots() {
            slot.node.stop();
        }
    }

    fn add_slot(&self) -> Result<AddedSlot, HostError> {
        let full = {
            let slots = self.slots.lock().unwrap();
            slots.entries.len() >= MAX_NETS || slots.created >= MAX_CREATED
        };
        if full {
            return Err(HostError::TooManyNets);
        }
        let run = run_for_net(&self.template, 0, false);
        let args = Args::parse_from(["web_demo", "--no-network"]);
        let node = Arc::new(crate::build_demo_node(&args, &run));
        let mut config = ConfigState::from_run(&run);
        config.overlay_process(&self.template.effective_settings.process);
        let id = self.push(node, config);
        Ok(AddedSlot { id, fresh: true })
    }

    fn retire_slot(&self, id: &NetSlotId, _announced: bool) -> Result<(), HostError> {
        if *id == NetSlotId::main() {
            return Ok(());
        }
        let mut slots = self.slots.lock().unwrap();
        let at = slots.entries.iter().position(|e| &e.handle.id == id);
        let entry = slots.entries.remove(at.ok_or(HostError::UnknownNet)?);
        entry.handle.node.stop();
        if let Some(task) = entry.task {
            task.abort();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "host_slot_tests.rs"]
mod slot_tests;
#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "host_tests_support.rs"]
mod tests_support;
