//! Tests for `ids.rs`: determinism and distinctness of ids.

use super::*;

#[test]
fn ids_are_deterministic_and_distinct() {
    assert_eq!(net_id(&[1; 32]), net_id(&[1; 32]));
    assert_ne!(net_id(&[1; 32]), net_id(&[2; 32]));
    assert_ne!(net_id(&[1; 32]), domains_id());
    assert_eq!(domains_id(), domains_id());
}

#[test]
fn topic_is_stable_and_differs_per_replica() {
    assert_eq!(topic_of(&net_id(&[1; 32])), topic_of(&net_id(&[1; 32])));
    assert_ne!(topic_of(&net_id(&[1; 32])), topic_of(&net_id(&[2; 32])));
    assert_ne!(topic_of(&net_id(&[1; 32])), *net_id(&[1; 32]).as_bytes());
}

#[test]
fn ids_golden() {
    // recompute: `b3sum --derive-key '<context>'` over 32 bytes 0x44 (net), over nothing (domains),
    // over the domains id (topic); contexts `meshtown replica {net,domains,topic} v1`
    let hex = |b: &[u8]| data_encoding::HEXLOWER.encode(b);
    assert_eq!(
        hex(net_id(&[0x44; 32]).as_bytes()),
        "cfd0ee213bba5486b889f89050bc123a749b28dedaad73b0ef1a4a67d3db29e8"
    );
    assert_eq!(
        hex(domains_id().as_bytes()),
        "447dbd2e86b5a8bb9ff3cd4a9b145118a977da80a19fa6335d0ca623f00e36a1"
    );
    assert_eq!(
        hex(&topic_of(&domains_id())),
        "a156adc6048f5ee0fcdf9c31c113c58e5d60e690676ba3db7a6f9151582403fb"
    );
}
