//! `redeem_with` closes the temporary `mem` transport made by `bind_ephemeral` when it exits.

use crate::invite::redeem::{redeem_with, RedeemError, RedeemOpts};
use crate::invite::test_responder::{head, ignore, opts, owner};
use crate::transport::net::facade::test_support::within;
use crate::transport::net::MemNet;
use crate::transport::net_mainline::unix_now;

#[tokio::test]
async fn ephemeral_mem_is_closed_on_exit() {
    // given: a temporary `mem` transport, the invite's deadline has already passed, no addresses
    let net = MemNet::new();
    let t = crate::node::bind_ephemeral(Some(&net))
        .await
        .expect("the temporary transport came up");
    let me = t.me();
    let expired = RedeemOpts {
        deadline_unix: unix_now().saturating_sub(10),
        ..opts()
    };
    // when
    let got = within(
        "redeem_with",
        redeem_with(t, &head(), &[], &expired, &ignore, &owner()),
    )
    .await;
    // then: Expired, the slot is closed by redeem_with itself
    assert!(matches!(got, Err(RedeemError::Expired)), "{got:?}");
    assert!(
        net.addr_of(me).is_none(),
        "the slot was not closed on the exit of redeem_with"
    );
}
