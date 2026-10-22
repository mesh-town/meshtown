//! Own synchronization on top of `crate::net`: topic membership by flooding (`flood`)
//! and own record replica (`replica`).
//! References only the network core; there is no node layer or foreign network libraries here.

mod framing;
pub mod flood;
pub mod replica;
