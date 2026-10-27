//! Restart in the same process and the fall back to waiting for a network after a failed start.

use crate::web_gui::ActionError;

use super::state::{Cmd, NodeCtl, NodeCtlState, Phase};

/// The saved start failure; only meaningful while the node waits in `NoNetwork`.
pub(super) fn start_error_of(st: &NodeCtlState) -> Option<String> {
    match st.phase {
        Phase::NoNetwork => st.start_error.clone(),
        _ => None,
    }
}

impl NodeCtl {
    /// Restart in the same process: from `Up`, or from `NoNetwork` after a failed start (the join is repeated
    /// with the saved key); `Cmd::Switch` with the current key (the join path rereads the config file and
    /// applies the CLI flags). Other phases — `Busy`.
    pub(super) fn request_restart(&self) -> Result<(), ActionError> {
        let key = {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match st.phase {
                Phase::Up => {}
                Phase::NoNetwork if st.start_error.is_some() => {}
                Phase::NoNetwork => {
                    return Err(ActionError::NoNetwork(super::gates::no_network_text()))
                }
                Phase::Starting => return Err(super::gates::busy_starting()),
                Phase::Stopping => return Err(super::gates::busy_stopping()),
            }
            let Some(key) = st.effective_settings.net.network_key.clone() else {
                return Err(ActionError::Internal("network key is unknown".to_string()));
            };
            st.phase = Phase::Starting;
            st.start_error = None;
            key
        };
        let _ = self.cmd_tx.send(Cmd::Switch { key, peers: vec![] });
        Ok(())
    }

    /// A start failed: `Starting` and `Stopping` become `NoNetwork` with the reason kept for the page; `Up`
    /// is left alone. A stop requested meanwhile stays in the command queue and is taken by the wait loop.
    pub(crate) fn fall_back_to_no_network(&self, reason: String) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if matches!(st.phase, Phase::Starting | Phase::Stopping) {
            st.phase = Phase::NoNetwork;
            st.start_error = Some(reason);
        }
    }

    /// Why the last start failed, while the node waits in `NoNetwork`; otherwise `None`.
    #[cfg(test)]
    pub(crate) fn start_error(&self) -> Option<String> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        start_error_of(&st)
    }
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;
