//! A command to the node poller from the menu items. Sent only by `Core`, read only by the `tray_role` poller.
//! A `slot` of `None` is the main network.

/// A command to the node poller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCmd {
    Start,
    Stop,
    /// Choose/clear an exit by the node's `id_full` in a network — a click on a submenu leaf.
    ToggleExit {
        slot: Option<String>,
        id: String,
    },
    /// Clear the chosen exit of a network — a click on "Directly".
    ClearExits(Option<String>),
    /// Turn `exit.enabled` of a network on or off — a click on the "Let members open sites" checkbox.
    SetExitServer {
        slot: Option<String>,
        on: bool,
    },
    /// Copy the invite of a network to the clipboard — a click on "Invite a member".
    CopyInvite(Option<String>),
}

/// Sender of commands to the node poller.
pub type NodeCmdTx = tokio::sync::mpsc::UnboundedSender<NodeCmd>;
