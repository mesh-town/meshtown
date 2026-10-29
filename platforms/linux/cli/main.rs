//! The `meshtown-linux` binary: the node with the tray icon of this OS.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    meshtown_core::autostart::install(&meshtown_linux::AUTOSTART);
    meshtown_core::cli::run::run_with_tray(&meshtown_linux::TRAY)
}

#[cfg(not(target_os = "linux"))]
fn main() {}
