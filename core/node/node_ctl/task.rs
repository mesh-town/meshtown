//! Background node snapshot task: once a second or sooner on a nudge it decides whether to skip a collection,
//! reads the `RunningNode` through `snapshot::collect` and puts the result into `state::NodeCtlState::cache`.

use std::sync::Arc;
use std::time::Duration;

use crate::node::RunningNode;

use super::snapshot;
use super::state::NodeCtlShared;

/// Loop of the background snapshot task: once a second (or sooner, on `kick`) it reads `effective_settings.node_name`
/// under the lock, decides whether to skip a collection (`last_read` is older than 5 s and
/// it is not the first tick after `attach`), picks the own name through [`snapshot_self_name`] (the effective
/// settings, or otherwise the name the node was started under), calls `collect`, and puts the result into `cache`
/// only if the generation has not changed (otherwise the task is outdated — `detach`/a new `attach`
/// overtakes it).
pub(super) async fn snapshot_task(
    ctl_shared: Arc<NodeCtlShared>,
    task_gen: u64,
    node: Arc<RunningNode>,
) {
    let mut is_first_tick = true;
    let mut doc_cache = snapshot::DocCache::default();
    loop {
        let (should_collect, name) = {
            let st = ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if st.snapshot_gen != task_gen {
                return;
            }
            let page_idle = st.last_read.elapsed() > Duration::from_secs(5);
            (
                is_first_tick || !page_idle,
                st.effective_settings.net.node_name.clone(),
            )
        };
        if should_collect {
            let self_name = snapshot_self_name(name.as_deref(), &node.name_at_start);
            let mut collected =
                snapshot::collect(&node, Some(self_name.as_str()), &mut doc_cache).await;
            let mut st = ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if st.snapshot_gen == task_gen {
                if collected.partial {
                    if let Some(prev) = &st.cache {
                        collected.domains = prev.domains.clone();
                    }
                }
                st.cache = Some(collected);
            }
        }
        is_first_tick = false;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = ctl_shared.kick.notified() => {}
        }
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
