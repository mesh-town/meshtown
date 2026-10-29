//! Binding outgoing sockets to a named network interface.
//!
//! The facade hides the per-OS mechanism: `SO_BINDTODEVICE` on Linux and Android,
//! `IP_BOUND_IF` on macOS, `IP_UNICAST_IF` on Windows. Other systems report
//! `Unsupported`, which callers treat as a refusal rather than a fallback to the default route.

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
mod unix_flags;

#[cfg_attr(any(target_os = "linux", target_os = "android"), path = "os_linux.rs")]
#[cfg_attr(target_os = "macos", path = "os_macos.rs")]
#[cfg_attr(target_os = "windows", path = "os_windows.rs")]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "windows")),
    path = "os_other.rs"
)]
mod os;
mod socket;
mod types;

#[cfg(test)]
pub(crate) mod test_support;

pub use socket::{bind_udp, check, connect_tcp, state_of};
pub use types::{BindError, ExitIfaceInfo, IfaceState, OutIface};
