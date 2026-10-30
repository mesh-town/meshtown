//! Drawing the tray view and the exits submenu: a change goes to the tray and into the log.

use crate::gui::tray::app::exit_menu::{disabled_exit_menu, ExitMenu};
use crate::gui::tray::app::view::{self, PageState, TrayColor, View};

use super::poller::Poller;

impl Poller {
    /// A view change: if the new one differs from the last, it goes to the tray and into the log;
    /// a grey view also turns the exits submenu off.
    pub(super) fn show(&mut self, view: View) {
        self.forget_start_note(view.node);
        self.show_changed_view(view);
        self.turn_exit_menu_off_when_grey(view);
    }

    /// Draws the view when it differs from the last one shown.
    fn show_changed_view(&mut self, view: View) {
        if self.last_view == Some(view) {
            return;
        }
        let previous = self.last_view.replace(view);
        self.apply_view(view, previous);
    }

    /// A grey view means the node is not running: the exits submenu is turned off in the same step, so there
    /// is no need to call `show_exits` separately on every grey view — all `View`s assembled by hand in the
    /// module are always grey, and green/red come only from `views_from_state_json`. A grey view of a ready
    /// page comes from a node reply too (every network is offline): its menu was sent just before and stays.
    fn turn_exit_menu_off_when_grey(&mut self, view: View) {
        if view.color != TrayColor::Grey || view.page == PageState::Ready {
            return;
        }
        self.show_exits(disabled_exit_menu(self.proxy.clone()));
    }

    /// Draws a changed view: it goes to the tray and into the log.
    fn apply_view(&mut self, view: View, previous: Option<View>) {
        (self.send_view)(view);
        self.say_view(view, previous);
    }

    /// A log line about a view change; a change of only the speed disc produces no line (the icon
    /// is redrawn, the log is not cluttered).
    fn say_view(&self, view: View, previous: Option<View>) {
        if previous.map(|p| View { disk: None, ..p }) == Some(View { disk: None, ..view }) {
            return;
        }
        self.say(format!(
            "{:?}; {}; {}",
            view.color,
            view::page_text(view),
            view::node_state_phrase(view.node)
        ));
    }

    /// The exits submenu: if the new one differs from the last, it goes to the tray.
    pub(super) fn show_exits(&mut self, menu: ExitMenu) {
        if self.last_exit_menu == Some(menu.clone()) {
            return;
        }
        self.last_exit_menu = Some(menu.clone());
        (self.send_exit_menu)(menu);
    }
}
