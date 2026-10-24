//! The `TrayBackend` of this OS: the event loop, the wake-up, opening a link, the clipboard and system notifications.

use meshtown_core::gui::tray::{copy_via, Core, TrayBackend};

use crate::{clipboard, notify, open_url, tray};

/// The macos tray backend.
pub struct MacosTray;

/// The handle to pass to `cli::run::run_with_tray`.
pub static TRAY: MacosTray = MacosTray;

impl TrayBackend for MacosTray {
    fn run(&self, core: Core) -> ! {
        tray::event_loop::run(core)
    }

    fn wake(&self) {
        tray::event_loop::wake()
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        open_url::open(url)
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), &'static str> {
        copy_via(clipboard::TOOLS, text)
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), &'static str> {
        notify::show(title, body)
    }
}
