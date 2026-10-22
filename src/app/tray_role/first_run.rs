//! First run: when the node reports no network yet, the tray "clicks" the "Open" item once per process.
//! The node page shows the setup wizard by itself while `/api/state` has `network: null`, so the plain page
//! address is enough. The click goes through the usual `TrayCmd::Menu` path, so the browser is opened on the
//! tray's main thread.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::app::view::{NetLink, PageState, View};
use crate::tray::{TrayCmd, MENU_OPEN};

/// A ready page whose node has no network: the setup wizard is what the user needs.
fn wants_wizard(view: &View) -> bool {
    view.page == PageState::Ready && view.net == NetLink::NoNetwork
}

/// The `send_view` of the poller: every view is shown; the first wizard-worthy one also opens the page.
/// The latch lives as long as the returned closure, i.e. the tray process.
pub(super) fn view_sender(send: impl Fn(TrayCmd) + Send + 'static) -> Box<dyn Fn(View) + Send> {
    let opened = AtomicBool::new(false);
    Box::new(move |view| {
        let wizard = wants_wizard(&view);
        send(TrayCmd::Update(view));
        if wizard && !opened.swap(true, Ordering::Relaxed) {
            send(TrayCmd::Menu(MENU_OPEN.to_string()));
        }
    })
}

#[cfg(test)]
#[path = "first_run_tests.rs"]
mod tests;
