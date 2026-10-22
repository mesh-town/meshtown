//! Tests of the unusable-address predicates (`unusable.rs`).

use super::*;

#[test]
fn fake_ip_and_loopback_are_unusable() {
    assert!(is_fake_ip(Ipv4Addr::new(198, 18, 0, 1)));
    assert!(is_fake_ip(Ipv4Addr::new(198, 19, 255, 254)));
    assert!(!is_fake_ip(Ipv4Addr::new(198, 20, 0, 1)));
    assert!(is_unusable(Ipv4Addr::new(127, 0, 0, 1)));
    assert!(is_unusable(Ipv4Addr::new(0, 0, 0, 0)));
    assert!(!is_unusable(Ipv4Addr::new(93, 184, 216, 34)));
}
