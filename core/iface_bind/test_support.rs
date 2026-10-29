//! Helpers shared by the interface-binding tests and the tests of its users.

/// An interface name that exists on no machine the tests run on.
pub(crate) const MISSING_IFACE: &str = "mtnone0";

/// Name of the first IPv4 loopback interface (`lo`, `lo0`, a friendly name on Windows).
pub(crate) fn loopback_name() -> String {
    let all = if_addrs::get_if_addrs().expect("interface list");
    let found = all
        .into_iter()
        .find(|i| i.is_loopback() && i.ip().is_ipv4());
    found.expect("an IPv4 loopback interface").name
}
