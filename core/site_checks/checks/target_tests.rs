//! Tests of choosing the exits to check through (`target.rs`).

use super::*;
use crate::transport::net::NodeSecret;

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

#[test]
fn exit_targets_only_online_exits_not_me() {
    let (me, on, off, gone, unknown) = (peer(1), peer(2), peer(3), peer(4), peer(5));
    let flags = HashMap::from([(me, true), (on, true), (off, false), (gone, true)]);
    let online = HashSet::from([me, on, off, unknown]);
    // `me` is dropped, `off` has the flag off, `gone` is offline, `unknown` has no flag.
    assert_eq!(exit_targets(&flags, &online, me), vec![on]);
}

#[test]
fn exit_targets_are_sorted() {
    let me = peer(9);
    let ids = [peer(7), peer(2), peer(5)];
    let flags: HashMap<_, _> = ids.iter().map(|p| (*p, true)).collect();
    let online: HashSet<_> = ids.iter().copied().collect();
    let got = exit_targets(&flags, &online, me);
    let mut want = ids.to_vec();
    want.sort();
    assert_eq!(got, want);
}
