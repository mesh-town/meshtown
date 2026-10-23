//! The `meshtown-windows` binary: the node with the tray icon of this OS.

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    meshtown_core::cli::run::run_with_tray(&meshtown_windows::TRAY)
}

#[cfg(not(target_os = "windows"))]
fn main() {}
