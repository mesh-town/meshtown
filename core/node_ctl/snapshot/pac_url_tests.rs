//! Test of the `socks.pac_url` field in the snapshot: the proxy listener gives the PAC address on the same port.
//! The helpers `base`/`pair` come from `super::tests`.

use super::tests::{base, pair};
use super::*;

#[test]
fn socks_listener_gives_pac_url_on_its_port() {
    // given
    let mut inputs = base(pair(1));
    inputs.socks = Some("127.0.0.1:41234".parse().unwrap());

    // when
    let collected = build(inputs);

    // then
    assert_eq!(
        collected.snap.socks.pac_url,
        Some("http://127.0.0.1:41234/proxy.pac".to_string())
    );
}
