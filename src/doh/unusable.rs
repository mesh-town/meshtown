//! Addresses a check must not use.

use std::net::Ipv4Addr;

/// An address from the fake-ip pool (RFC 2544, `198.18.0.0/15`): issued by our own TUN.
pub fn is_fake_ip(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 198 && (o[1] == 18 || o[1] == 19)
}

/// An address a check must not use: our own TUN, loopback, "zero", link-local, broadcast.
pub fn is_unusable(ip: Ipv4Addr) -> bool {
    is_fake_ip(ip)
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
}

#[cfg(test)]
#[path = "unusable_tests.rs"]
mod tests;
