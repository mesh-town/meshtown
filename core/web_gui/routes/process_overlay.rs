//! The process block of an added slot: the primary slot's, shown by every route that reads the slot's config.

use crate::web_gui::{App, SlotHandle};

/// For an added slot: its config state shows the primary slot's process block. Idempotent; the primary slot
/// is left alone.
pub(super) fn overlay_from_main(app: &App, slot: &SlotHandle) {
    if slot.id.carries_process() {
        return;
    }
    let process = {
        let main = app.host.main_slot();
        let cfg = main.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.file_settings.process.clone()
    };
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.overlay_process(&process);
}
