//! Own synchronization on top of `crate::transport::net`: topic membership by flooding (`flood`)
//! and own record replica (`replica`).
//! References only the network core; there is no node layer or foreign network libraries here.

pub mod flood;
mod framing;
pub mod replica;
