//! Questions the node cycle asks the control object between two starts.

use super::state::Phase;
use super::NodeCtl;

impl NodeCtl {
    /// A stop was requested and not yet carried out. After `detach(Starting)` the phase is `Starting`,
    /// so `true` means the request arrived while the node was restarting.
    pub(crate) fn stop_requested(&self) -> bool {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        matches!(st.phase, Phase::Stopping)
    }
}
