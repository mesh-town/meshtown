//! Tests of the proxy menu item text (`proxy_item_text`).

use super::*;
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};

fn on(addr: &str) -> ProxyState {
    ProxyState::On(addr.to_string())
}

#[test]
fn proxy_item_text_off_by_settings() {
    let off = ProxyState::Off(ProxyOff::DisabledInSettings);
    assert_eq!(proxy_item_text(&off), "Proxy is off in the settings file");
}

#[test]
fn proxy_item_text_off_by_flag() {
    let off = ProxyState::Off(ProxyOff::NoSocksFlag);
    assert_eq!(
        proxy_item_text(&off),
        "Proxy is off until Mesh Town restarts"
    );
}

#[test]
fn proxy_item_text_ipv4() {
    let addr = on("127.0.0.1:1080");
    assert_eq!(
        proxy_item_text(&addr),
        "Browser proxy: 127.0.0.1, port 1080 (SOCKS5 or HTTP, no password)"
    );
}

#[test]
fn proxy_item_text_ipv6() {
    let addr = on("[::1]:1080");
    assert_eq!(
        proxy_item_text(&addr),
        "Browser proxy: ::1, port 1080 (SOCKS5 or HTTP, no password)"
    );
}

#[test]
fn proxy_item_text_no_port_falls_back() {
    let addr = on("localhost");
    assert_eq!(
        proxy_item_text(&addr),
        "Browser proxy: localhost (SOCKS5 or HTTP)"
    );
}

#[test]
fn quit_text_is_an_action() {
    assert_eq!(quit_text(), "Quit Mesh Town");
}

fn click_event(button_state: tray_icon::MouseButtonState) -> tray_icon::TrayIconEvent {
    tray_icon::TrayIconEvent::Click {
        id: tray_icon::TrayIconId::new("t"),
        position: Default::default(),
        rect: Default::default(),
        button: tray_icon::MouseButton::Left,
        button_state,
    }
}

fn plain_event(
    make: fn(
        tray_icon::TrayIconId,
        tray_icon::dpi::PhysicalPosition<f64>,
        tray_icon::Rect,
    ) -> tray_icon::TrayIconEvent,
) -> tray_icon::TrayIconEvent {
    make(
        tray_icon::TrayIconId::new("t"),
        Default::default(),
        Default::default(),
    )
}

#[test]
fn is_press_only_on_button_down() {
    use tray_icon::{MouseButtonState, TrayIconEvent};
    assert!(is_press(&click_event(MouseButtonState::Down)));
    assert!(!is_press(&click_event(MouseButtonState::Up)));
    let enter = plain_event(|id, position, rect| TrayIconEvent::Enter { id, position, rect });
    let moved = plain_event(|id, position, rect| TrayIconEvent::Move { id, position, rect });
    assert!(!is_press(&enter));
    assert!(!is_press(&moved));
}
