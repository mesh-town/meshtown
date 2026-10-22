//! Whether a tray icon is needed at all.
//!
//! What it does: decides whether the `tray` process works without an icon — because the platform itself gives
//! no graphical session (no desktop access on macOS, no `DISPLAY`/`WAYLAND_DISPLAY` on
//! Linux, session 0 on Windows).
//! What it doesn't do: it doesn't decide what to print — the caller (`tray_role`) builds
//! the line `[tray] no tray icon: <reason>` from the returned text.
//! Links: the deciding logic (`graphic_access_from_bits`, `headless_from_display`,
//! `is_headless_session`) is moved into pure functions without `cfg` so the host (macOS) tests it
//! for the Linux/Windows branches that aren't built here; the system call itself on each
//! platform is a thin `extern` wrapper without new crates (this module doesn't touch `Cargo.toml`).

/// The reason to work without a tray icon, if there is one.
pub fn headless_reason() -> Option<&'static str> {
    platform_reason()
}

/// Whether there is access to a graphical session, by the call status and the macOS attribute bits of
/// `SessionGetInfo`: `noErr` (0) and the `sessionHasGraphicAccess` bit set.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // pure logic of another platform: the host tests call it
fn graphic_access_from_bits(status: i32, attributes: u32) -> bool {
    const SESSION_HAS_GRAPHIC_ACCESS: u32 = 0x0010;
    status == 0 && attributes & SESSION_HAS_GRAPHIC_ACCESS != 0
}

/// A headless session on Linux — when neither `DISPLAY` nor `WAYLAND_DISPLAY` is set (an empty string
/// counts as absent — most graphical toolkits behave the same).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // pure logic of another platform: the host tests call it
fn headless_from_display(display: Option<&str>, wayland: Option<&str>) -> bool {
    let empty = |v: Option<&str>| v.map(str::is_empty).unwrap_or(true);
    empty(display) && empty(wayland)
}

/// A headless session on Windows — session 0 (a service one, with no interactive desktop).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))] // pure logic of another platform: the host tests call it
fn is_headless_session(session_id: u32) -> bool {
    session_id == 0
}

#[cfg(target_os = "macos")]
mod macos_ffi {
    #[link(name = "Security", kind = "framework")]
    extern "C" {
        pub fn SessionGetInfo(
            requested_session: u32,
            session_id: *mut u32,
            attributes: *mut u32,
        ) -> i32;
    }
}

#[cfg(target_os = "macos")]
fn platform_reason() -> Option<&'static str> {
    // `callerSecuritySession` (Security/AuthSession.h) — 0xffff_ffff.
    const CALLER_SECURITY_SESSION: u32 = 0xffff_ffff;
    let mut session_id: u32 = 0;
    let mut attributes: u32 = 0;
    let status = unsafe {
        macos_ffi::SessionGetInfo(CALLER_SECURITY_SESSION, &mut session_id, &mut attributes)
    };
    if graphic_access_from_bits(status, attributes) {
        None
    } else {
        Some("no access to a graphical session (started over SSH or as a service)")
    }
}

#[cfg(target_os = "windows")]
mod windows_ffi {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetCurrentProcessId() -> u32;
        pub fn ProcessIdToSessionId(process_id: u32, session_id: *mut u32) -> i32;
    }
}

#[cfg(target_os = "windows")]
fn platform_reason() -> Option<&'static str> {
    let pid = unsafe { windows_ffi::GetCurrentProcessId() };
    let mut session_id: u32 = 0;
    let ok = unsafe { windows_ffi::ProcessIdToSessionId(pid, &mut session_id) };
    if ok == 0 || is_headless_session(session_id) {
        Some("session 0 or a ProcessIdToSessionId error (a service without a desktop)")
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn platform_reason() -> Option<&'static str> {
    let display = std::env::var("DISPLAY").ok();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    if headless_from_display(display.as_deref(), wayland.as_deref()) {
        Some("no DISPLAY and WAYLAND_DISPLAY")
    } else {
        None
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_reason() -> Option<&'static str> {
    Some("the platform doesn't support an icon in the system tray")
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
