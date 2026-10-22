//! Words for the "node unknown" view (`NodeState::Unknown`): why the icon cannot see the node.

use super::PageState;
use crate::gui::tray::app::tray_texts::{
    status_page_port_busy_text, status_unresponsive_text, STATUS_PAGE_DISABLED_TEXT,
};

/// The first menu line for `NodeState::Unknown`: the reason comes from the page state.
pub(super) fn unknown_status(page: PageState) -> String {
    match page {
        PageState::PortBusy(port) => status_page_port_busy_text(port),
        PageState::Disabled => STATUS_PAGE_DISABLED_TEXT.to_string(),
        _ => status_unresponsive_text(),
    }
}
