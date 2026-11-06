use std::sync::Arc;

use super::*;
use crate::transport::net::NodeSecret;

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

#[test]
fn exit_pair_not_started_twice_while_running() {
    // given: a pair whose check is running
    let pairs = Arc::new(PairsInProgress::default());
    let first = pairs.try_begin("a.test", peer(1));
    assert!(first.is_some());

    // when/then: the same pair does not start; another pair does
    assert!(pairs.try_begin("a.test", peer(1)).is_none());
    assert!(pairs.try_begin("a.test", peer(2)).is_some());
    assert!(pairs.try_begin("b.test", peer(1)).is_some());

    // after the first one ends, the pair starts again
    drop(first);
    assert!(pairs.try_begin("a.test", peer(1)).is_some());
}
