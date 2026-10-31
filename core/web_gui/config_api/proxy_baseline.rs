//! The baseline of the `socks.*` fields follows the listener the node reports, not the file of the last lap.

use crate::proxy::ProxyFace;
use crate::web_gui::NodeControl;

use super::state::ConfigState;

impl ConfigState {
    /// Moves the `socks.*` baseline to what the process proxy listens on now, so that `restart_pending` for them
    /// means "the file against the real listener". Launch flags on `socks.*` keep the launch baseline, and an
    /// added slot keeps the primary slot's block it was given: it cannot restart the process proxy.
    pub(crate) fn sync_proxy_baseline(&mut self, node: &dyn NodeControl) {
        if self.process_overlaid || self.launch_flag_on_socks() {
            return;
        }
        let Some(face) = node.applied_proxy() else {
            return;
        };
        self.follow_face(&face);
    }

    fn launch_flag_on_socks(&self) -> bool {
        self.cli_override_paths
            .iter()
            .any(|p| p.starts_with("socks."))
    }

    fn follow_face(&mut self, face: &ProxyFace) {
        let boot = &mut self.boot_file_settings.process;
        boot.socks_enabled = face.addr.is_some();
        let Some(addr) = face.addr else { return };
        boot.socks_port = addr.port();
        boot.socks_access = face.access;
        boot.socks_ifaces = face.ifaces.clone();
    }
}
