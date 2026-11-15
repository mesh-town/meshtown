//! The "browser is connected" page of the bare `meshtown` name.

use super::page::{MeshPage, SharedDevice};
use super::page_html::{esc, fill, para, port_line, Parts};
use super::page_texts::{
    PAGE_H1_CONNECTED, PAGE_H2_CONNECTED_PORTS, PAGE_LEAD_CONNECTED, PAGE_NOBODY_SHARED,
    PAGE_OFFLINE, PAGE_ONLINE, PAGE_TITLE_CONNECTED,
};

/// Status 200. Unlike the error pages it has no "who speaks" line: it is the answer itself.
pub(super) fn connected(page: &MeshPage, devices: &[SharedDevice]) -> Parts {
    let lead = para("", &fill(PAGE_LEAD_CONNECTED, &[("me", page.me.as_str())]));
    let body = format!(
        "<h1>{PAGE_H1_CONNECTED}</h1>{lead}<h2>{PAGE_H2_CONNECTED_PORTS}</h2>{}",
        device_rows(devices)
    );
    Parts {
        status: 200,
        title: PAGE_TITLE_CONNECTED.to_string(),
        body,
    }
}

fn device_rows(devices: &[SharedDevice]) -> String {
    if devices.is_empty() {
        return para("", &esc(PAGE_NOBODY_SHARED));
    }
    devices.iter().map(device_row).collect()
}

fn device_row(device: &SharedDevice) -> String {
    let badge = if device.online {
        PAGE_ONLINE
    } else {
        PAGE_OFFLINE
    };
    let name = format!(
        "<div><strong>{}</strong><span class=\"badge\">{badge}</span></div>",
        esc(&device.name)
    );
    let ports: String = device.open.iter().map(port_line).collect();
    format!("<div class=\"card\">{name}<ul>{ports}</ul></div>")
}
