//! The phase gates of `NodeCtl`: which phase lets a page request reach the running node and which answers
//! "busy". Reading the membership list needs only the node; actions also wait for the first snapshot.

use std::sync::Arc;

use crate::node::output::NET_NAME;
use crate::node::RunningNode;
use crate::web_gui::ActionError;

use super::state::{NodeCtl, Phase};

/// "This computer is not on a network yet" — the text of the `NoNetwork` phase.
pub(super) fn no_network_text() -> String {
    format!(
        "this computer has not joined the {NET_NAME} network yet: paste an invite from a network member or create your own"
    )
}

pub(super) fn starting_text() -> String {
    format!("{NET_NAME} is starting")
}

pub(super) fn stopping_text() -> String {
    format!("{NET_NAME} is stopping")
}

pub(super) fn busy_starting() -> ActionError {
    ActionError::Busy(starting_text())
}

pub(super) fn busy_stopping() -> ActionError {
    ActionError::Busy(stopping_text())
}

impl NodeCtl {
    /// Action gates: `NoNetwork` → the computer is not on a network yet; `Starting`, or `Up` without a ready
    /// snapshot → busy "starting"; `Stopping` → busy "stopping"; otherwise a clone of the node.
    pub(super) fn running_node_for_action(&self) -> Result<Arc<RunningNode>, ActionError> {
        self.gate(true)
    }

    /// Read gate: like the action gate, but `Up` is enough — the list does not need the first snapshot.
    pub(super) fn running_node_for_read(&self) -> Result<Arc<RunningNode>, ActionError> {
        self.gate(false)
    }

    fn gate(&self, need_snapshot: bool) -> Result<Arc<RunningNode>, ActionError> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match st.phase {
            Phase::NoNetwork => Err(ActionError::Busy(no_network_text())),
            Phase::Starting => Err(busy_starting()),
            Phase::Stopping => Err(busy_stopping()),
            Phase::Up if need_snapshot && st.cache.is_none() => Err(busy_starting()),
            Phase::Up => Ok(st.node.clone().expect("Phase::Up always means node = Some")),
        }
    }
}
