//! The process-wide proxy entrance: one listener that serves the network chosen for the proxy.
//!
//! Files: `listen` — the listener and its target (`ProxyFront`); `face` — what the listener shows to
//! the outside (`ProxyFace`) and its journal lines; `seat` — which network's configuration is the target
//! (`ProxySeat`); `nets` — the directory of the running networks' configurations.

mod face;
mod listen;
mod nets;
mod seat;

pub use face::ProxyFace;
pub(super) use nets::NetDirectory;
pub(crate) use nets::NetEntry;
pub use nets::MeshAddress;
pub use listen::ProxyFront;
pub use seat::ProxySeat;
