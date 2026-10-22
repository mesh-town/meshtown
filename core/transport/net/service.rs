//! Service identifier (an ALPN-like stream label) and the project's three constants.
//! The service protocols themselves are not here.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ServiceId(&'static [u8]);

pub const AUTH: ServiceId = ServiceId::new(b"meshtown/auth/1");
pub const EXIT: ServiceId = ServiceId::new(b"meshtown/exit/1");
pub const PORT: ServiceId = ServiceId::new(b"meshtown/port/1");
pub const INVITE: ServiceId = ServiceId::new(b"meshtown/invite/1");
pub const SYNC: ServiceId = ServiceId::new(b"meshtown/sync/1");
pub const REPLICA: ServiceId = ServiceId::new(b"meshtown/replica/1");

impl ServiceId {
    pub const fn new(bytes: &'static [u8]) -> Self {
        ServiceId(bytes)
    }

    pub fn as_bytes(&self) -> &'static [u8] {
        self.0
    }
}

#[cfg(all(test, feature = "net-hyper"))]
#[path = "service_tests.rs"]
mod tests;
