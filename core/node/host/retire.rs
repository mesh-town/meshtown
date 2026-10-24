//! Leaving a network: the slot leaves the map at once; the node's stop and the move of its folder (or of the
//! main file) go on in a background task that the host waits for at the end of the run.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, MutexGuard};
use std::time::Duration;

use tokio::sync::watch;

use super::slots::{HostSlots, Slot};
use crate::config::config_file::{
    net_slot_path, new_secret, set_aside_main, set_aside_slot, NetSlotId, SaveError,
};
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{self, say};
use crate::transport::net::NodeSecret;
use crate::web_gui::{now_unix, HostControl, HostError, SlotHandle};

/// How long a node that told its neighbours about leaving keeps running, so the record reaches them.
pub const LEAVE_GRACE: Duration = Duration::from_secs(3);
/// How long the node gets to stop (or to drop its network) before the files are left untouched.
const LEAVE_STOP_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(100);

const KEY_FROM_FLAG: &str =
    "the network key comes from --key or MESHTOWN_KEY: restart without it to leave";
const RETURNS: &str = "the network returns on next start";

impl HostSlots {
    /// Takes the network away from the page; the stop and the files follow in the background.
    pub(super) fn retire(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError> {
        if id.carries_process() {
            return self.leave_in_place(id, announced);
        }
        let mut tasks = self.retiring_tasks();
        if self.is_stopping() {
            return Err(HostError::Busy("the node is stopping".into()));
        }
        let claim = self.claim_guard();
        let task = match self.remove_slot(id) {
            Some(slot) => self.leave_and_remove(id, slot, &claim, announced),
            None if self.take_broken(id) => {
                leave_task(id, set_aside_folder(id.clone(), self.main_file()))
            }
            None => return Err(HostError::UnknownNet),
        };
        tasks.push(task);
        Ok(())
    }

    /// The background task of a slot that has left the map.
    fn leave_and_remove(
        &self,
        id: &NetSlotId,
        slot: Slot,
        claim: &MutexGuard<'_, ()>,
        announced: bool,
    ) -> tokio::task::JoinHandle<()> {
        self.leave_proxy_seat(id, claim);
        let main_file = self.main_file();
        let work = retire_added(id.clone(), slot.ctl, slot.ended, main_file, announced);
        leave_task(id, work)
    }

    fn leave_in_place(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError> {
        let slot = self.slot(id).ok_or(HostError::UnknownNet)?;
        let (has_key, from_flag) = {
            let state = slot.config.lock().unwrap_or_else(|e| e.into_inner());
            (
                state.current_network_key.is_some(),
                state.cli_override_paths.contains(&"network.key"),
            )
        };
        if from_flag {
            return Err(HostError::Busy(KEY_FROM_FLAG.into()));
        }
        if !has_key {
            return Ok(());
        }
        let secret = new_secret().map_err(|e| HostError::Io(format!("{e:#}")))?;
        let mut tasks = self.retiring_tasks();
        if self.is_stopping() {
            return Err(HostError::Busy("the node is stopping".into()));
        }
        let ctl = self.ctl_of(id).ok_or(HostError::UnknownNet)?;
        if self.leaving.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let flag = LeavingFlag(self.leaving.clone());
        let task = leave_in_place_task(slot, ctl, secret, announced, flag);
        tasks.push(leave_task(id, task));
        Ok(())
    }

    /// The path of the main settings file, when the run has one.
    fn main_file(&self) -> Option<PathBuf> {
        let main = self.main_slot();
        let state = main.config.lock().unwrap_or_else(|e| e.into_inner());
        state.file.as_ref().map(|f| f.path().to_path_buf())
    }
}

/// Runs `work` as a task labelled with the slot id.
fn leave_task(
    id: &NetSlotId,
    work: impl Future<Output = ()> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    output::spawn(output::with_net(Some(Arc::from(id.to_string())), work))
}

async fn retire_added(
    id: NetSlotId,
    ctl: Arc<NodeCtl>,
    ended: Option<watch::Receiver<bool>>,
    main_file: Option<PathBuf>,
    announced: bool,
) {
    if announced {
        tokio::time::sleep(LEAVE_GRACE).await;
    }
    ctl.stop_for_leave();
    if !wait_ended(ended).await {
        let why = "the node did not stop in time";
        say(
            "host",
            format!("network folder not moved: {why}; {RETURNS}"),
        );
        return;
    }
    set_aside_folder(id, main_file).await;
}

/// `true` when the cycle ended in time; a closed sender (a panicked cycle) counts as the end.
async fn wait_ended(ended: Option<watch::Receiver<bool>>) -> bool {
    let Some(mut ended) = ended else {
        return true;
    };
    let waited = tokio::time::timeout(LEAVE_STOP_WAIT, ended.wait_for(|done| *done)).await;
    waited.is_ok()
}

/// Moves the slot's folder to `nets/left-…`; a slot that never got a key has no folder and nothing to move.
async fn set_aside_folder(id: NetSlotId, main_file: Option<PathBuf>) {
    let Some(main) = main_file else {
        return;
    };
    if !has_folder(&main, &id) {
        return;
    }
    match set_aside_slot(&main, &id, now_unix()) {
        Ok(to) => say(
            "host",
            format!("network {id} set aside to {}", to.display()),
        ),
        Err(e) => say("host", format!("network folder not moved: {e}; {RETURNS}")),
    }
}

fn has_folder(main: &Path, id: &NetSlotId) -> bool {
    let file = net_slot_path(main, id);
    file.and_then(|f| f.parent().map(Path::is_dir))
        .unwrap_or(false)
}

/// Clears the "leave in progress" flag of the host when the leave task ends, however it ends.
struct LeavingFlag(Arc<AtomicBool>);

impl Drop for LeavingFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn leave_in_place_task(
    slot: SlotHandle,
    ctl: Arc<NodeCtl>,
    secret: NodeSecret,
    announced: bool,
    _leaving: LeavingFlag,
) {
    if announced {
        tokio::time::sleep(LEAVE_GRACE).await;
    }
    if let Err(e) = ctl.request_leave_net(secret.clone()) {
        say("host", format!("couldn't leave the network: {e:?}"));
        return;
    }
    if !wait_no_network(&ctl).await {
        say(
            "host",
            "the node did not leave its network in time; nothing was changed",
        );
        return;
    }
    let file = slot
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .file
        .as_ref()
        .map(|f| f.path().to_path_buf());
    let kept = file.is_some_and(|path| !set_aside_main_file(&path));
    let mut state = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = state.drop_network(secret, now_unix(), kept) {
        say(
            "host",
            format!("couldn't write the settings file: {}", describe(&e)),
        );
    }
}

/// Copies the main file and its caches to `nets/left-main-…`; `false` when nothing was moved because of a failure.
fn set_aside_main_file(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    match set_aside_main(path, now_unix()) {
        Ok(to) => {
            say("host", format!("the network set aside to {}", to.display()));
            true
        }
        Err(e) => {
            say("host", format!("network files not moved: {e}; {RETURNS}"));
            false
        }
    }
}

async fn wait_no_network(ctl: &NodeCtl) -> bool {
    let polling = async {
        while !ctl.is_no_network() {
            tokio::time::sleep(POLL).await;
        }
    };
    tokio::time::timeout(LEAVE_STOP_WAIT, polling).await.is_ok()
}

fn describe(e: &SaveError) -> String {
    match e {
        SaveError::StaleRev(_) => "the file changed during the write".to_string(),
        SaveError::Io(text) => text.clone(),
    }
}

#[cfg(test)]
#[path = "retire_tests.rs"]
mod tests;
