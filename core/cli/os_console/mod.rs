//! Declarations of the submodules for console output by process role. The shared logic (role parsing, the log,
//! rotation) is in `log.rs`; platform behaviour is in the `///` above `init` in `windows.rs`/`macos.rs`.

mod log;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(not(any(windows, target_os = "macos")))]
pub use log::init;
#[cfg(target_os = "macos")]
pub use macos::init;
#[cfg(windows)]
pub use windows::init;

#[cfg(feature = "tray")]
pub use log::node_log_tail;
#[cfg(all(feature = "tray", any(windows, target_os = "macos")))]
pub use log::{child_needs_node_log, open_node_log};
