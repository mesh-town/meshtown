//! The process-wide proxy entrance: one listener that serves the network chosen for the proxy.
//!
//! Files: `listen` — the listener and its target (`ProxyFront`); `face` — what the listener shows to
//! the outside (`ProxyFace`) and its journal lines; `seat` — which network's configuration is the target
//! (`ProxySeat`).

mod face;
mod listen;
mod seat;

pub use face::ProxyFace;
pub use listen::ProxyFront;
pub use seat::ProxySeat;
