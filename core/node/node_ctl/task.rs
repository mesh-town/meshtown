//! Background node snapshot task: once a second or sooner on a nudge it decides whether to skip a collection,
//! reads the `RunningNode` through `snapshot::collect` and puts the result into `state::NodeCtlState::cache`.

use std::sync::Arc;
use std::time::Duration;

use crate::node::RunningNode;

use super::snapshot::{self, Collected};
use super::state::NodeCtlShared;

/// Loop of the background snapshot task: [`decide_snapshot`] under the lock, [`snapshot_self_name`] and `collect`
/// outside it, [`store_snapshot`] under the lock, then [`wait_tick`]. The task ends when the generation changes
/// (`detach`/a new `attach` overtakes it).
pub(super) async fn snapshot_task(
    ctl_shared: Arc<NodeCtlShared>,
    task_gen: u64,
    node: Arc<RunningNode>,
) {
    let mut is_first_tick = true;
    let mut doc_cache = snapshot::DocCache::default();
    loop {
        let Some((should_collect, name)) = decide_snapshot(&ctl_shared, task_gen, is_first_tick)
        else {
            return;
        };
        if should_collect {
            let self_name = snapshot_self_name(name.as_deref(), &node.name_at_start);
            let collected =
                snapshot::collect(&node, Some(self_name.as_str()), &mut doc_cache).await;
            store_snapshot(&ctl_shared, task_gen, collected);
        }
        is_first_tick = false;
        wait_tick(&ctl_shared).await;
    }
}

/// Under the lock: `None` if the generation changed; otherwise whether to collect (first tick or the page was read
/// within the last 5 s) and the effective node name.
fn decide_snapshot(
    ctl_shared: &NodeCtlShared,
    task_gen: u64,
    is_first_tick: bool,
) -> Option<(bool, Option<String>)> {
    let st = ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if st.snapshot_gen != task_gen {
        return None;
    }
    let page_idle = st.last_read.elapsed() > Duration::from_secs(5);
    Some((
        is_first_tick || !page_idle,
        st.effective_settings.net.node_name.clone(),
    ))
}

/// A partial collection keeps the domain list of the previous one, if there is one.
fn with_previous_domains(mut collected: Collected, previous: Option<&Collected>) -> Collected {
    if let (true, Some(prev)) = (collected.partial, previous) {
        collected.domains = prev.domains.clone();
    }
    collected
}

/// Under the lock: puts the result into `cache` only if the generation has not changed.
fn store_snapshot(ctl_shared: &NodeCtlShared, task_gen: u64, collected: Collected) {
    let mut st = ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if st.snapshot_gen == task_gen {
        st.cache = Some(with_previous_domains(collected, st.cache.as_ref()));
    }
}

/// Waits one second or until `kick`.
async fn wait_tick(ctl_shared: &NodeCtlShared) {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        _ = ctl_shared.kick.notified() => {}
    }
}

/// The own name for `snapshot::collect`: the effective settings (`effective_settings.node_name`) if set,
/// otherwise the name the node actually runs under (`RunningNode.name_at_start`) — the snapshot is never
/// left without a name.
pub(super) fn snapshot_self_name(effective_name: Option<&str>, running: &str) -> String {
    effective_name.unwrap_or(running).to_string()
}

#[cfg(test)]
#[path = "task_tests.rs"]
mod tests;
