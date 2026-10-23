//! Starting the networks saved under `nets/` beside the main settings file: every slot folder that can be
//! read is brought up next to the main slot, the rest is reported in the log and left untouched.

use super::launch::SlotLaunch;
use super::slots::HostSlots;
use crate::config::config_file::{list_net_slots, prepare_net_slot, NetSlotId, RunConfig};
use crate::members::network_key::{parse_user_input, NetworkKeyInput};
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::Relaunch;
use crate::node::output::say;
use crate::node::IdentityState;

/// How many networks one process runs; the main slot always counts, even without a key.
pub const MAX_NETS: usize = 8;

/// The networks already running, to refuse a copied slot folder.
struct Running(Vec<(String, NetSlotId)>);

impl Running {
    fn of_main(main: &RunConfig) -> Running {
        let key = main.effective_settings.net.network_key.as_deref();
        let id = key
            .and_then(|k| parse_user_input(k).ok())
            .map(|p| p.key.subkeys().network_id_hex());
        Running(id.map(|i| (i, NetSlotId::main())).into_iter().collect())
    }

    fn slot_of(&self, network: &str) -> Option<&NetSlotId> {
        self.0.iter().find(|(n, _)| n == network).map(|(_, id)| id)
    }
}

/// What starts a prepared slot: its own launch settings and the parsed key.
fn slot_launch(
    id: NetSlotId,
    run: RunConfig,
    parsed: NetworkKeyInput,
    relaunch: &Relaunch,
) -> SlotLaunch {
    let relaunch = relaunch.for_net_slot(&run);
    let mut cfg = relaunch.node_config(&run.effective_settings, parsed.key);
    cfg.network_label = parsed.label;
    cfg.identity = IdentityState::from_report(&run.file_report);
    SlotLaunch {
        id,
        run,
        relaunch,
        start: Some(cfg),
    }
}

fn not_started(id: &NetSlotId, why: impl std::fmt::Display) {
    say("node", format!("net slot {id} not started: {why}"));
}

/// The parsed key of a prepared slot; the text is for the log and holds no key value.
fn slot_key(run: &RunConfig) -> Result<NetworkKeyInput, String> {
    let text = run.effective_settings.net.network_key.as_deref();
    let text = text.ok_or_else(|| "the file has no network key".to_string())?;
    parse_user_input(text).map_err(|_| "the network key could not be parsed".to_string())
}

/// Whether the main settings file has saved network folders beside it; without a file (`--no-config`) none.
pub(crate) fn has_saved_slots(main: Option<&RunConfig>) -> bool {
    let file = main.and_then(|m| m.file.as_ref());
    file.is_some_and(|f| !list_net_slots(f.path()).is_empty())
}

/// The network label is shown in the log only while more than one network runs.
pub(crate) fn multi_net_for(running: usize) -> bool {
    running > 1
}

impl HostSlots {
    /// Loads and starts the saved slots after the main slot's task is running. It reports problems in the log
    /// and never returns an error. Without a main settings file (`--no-config`) `nets/` is not read.
    pub(crate) fn start_saved_slots(
        &self,
        main: &RunConfig,
        relaunch: &Relaunch,
        sources: &GuardSources,
        now_unix: u64,
    ) {
        let Some(file) = main.file.as_ref() else {
            return;
        };
        let mut running = Running::of_main(main);
        for slot in list_net_slots(file.path()) {
            let id = slot.id.clone();
            match prepare_net_slot(main, slot, now_unix) {
                Err(why) => {
                    not_started(&id, why);
                    self.mark_broken(id);
                }
                Ok(run) => self.start_prepared(id, run, relaunch, sources, &mut running),
            }
        }
    }

    fn start_prepared(
        &self,
        id: NetSlotId,
        run: RunConfig,
        relaunch: &Relaunch,
        sources: &GuardSources,
        running: &mut Running,
    ) {
        let parsed = match slot_key(&run) {
            Ok(p) => p,
            Err(why) => {
                not_started(&id, why);
                return self.mark_broken(id);
            }
        };
        let network = parsed.key.subkeys().network_id_hex();
        if let Some(other) = running.slot_of(&network) {
            return not_started(&id, format!("same network as slot {other}"));
        }
        if self.len() >= MAX_NETS {
            return not_started(
                &id,
                format!("limit of {MAX_NETS} networks; the folder is kept"),
            );
        }
        match self.launch(slot_launch(id.clone(), run, parsed, relaunch), sources) {
            Ok(()) => running.0.push((network, id)),
            Err(why) => not_started(&id, format!("{why:?}")),
        }
    }
}
