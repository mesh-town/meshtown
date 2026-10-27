//! Questions the node cycle asks the control object between two starts.

use crate::node::output::say;
use crate::transport::net::NodeSecret;
use crate::web_gui::ActionError;

use super::state::{Cmd, Phase};
use super::NodeCtl;

impl PartialEq for Cmd {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Cmd::Stop, Cmd::Stop) => true,
            (Cmd::LeaveNet { secret: a }, Cmd::LeaveNet { secret: b }) => {
                a.peer_id() == b.peer_id()
            }
            (
                Cmd::Switch {
                    key: k1,
                    peers: p1,
                    secret: s1,
                },
                Cmd::Switch {
                    key: k2,
                    peers: p2,
                    secret: s2,
                },
            ) => {
                k1 == k2
                    && p1 == p2
                    && s1.as_ref().map(NodeSecret::peer_id) == s2.as_ref().map(NodeSecret::peer_id)
            }
            _ => false,
        }
    }
}

impl Eq for Cmd {}

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

    /// Leaving the network the node is on: from `Stopping` — `Busy`; from `NoNetwork` there is nothing to
    /// leave (`Ok`, no command); otherwise `Cmd::LeaveNet` with the identity the node takes afterwards.
    pub(crate) fn request_leave_net(&self, secret: NodeSecret) -> Result<(), ActionError> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match st.phase {
            Phase::Stopping => return Err(super::gates::busy_stopping()),
            Phase::NoNetwork => return Ok(()),
            Phase::Starting | Phase::Up => {}
        }
        drop(st);
        let _ = self.cmd_tx.send(Cmd::LeaveNet { secret });
        Ok(())
    }

    /// The node waits for a network key.
    pub(crate) fn is_no_network(&self) -> bool {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        matches!(st.phase, Phase::NoNetwork)
    }

    /// Stops the node because its network is being left.
    pub(crate) fn stop_for_leave(&self) {
        say("node", "leaving the network");
        self.request_stop();
    }
}

#[cfg(test)]
#[path = "leave_net_tests.rs"]
mod tests;
