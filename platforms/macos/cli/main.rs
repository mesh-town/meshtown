//! The `meshtown-macos` binary: the node with the tray icon of this OS.

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    meshtown_core::cli::run::run_with_tray(&meshtown_macos::TRAY)
}

#[cfg(not(target_os = "macos"))]
fn main() {}
