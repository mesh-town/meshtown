//! `redeem_for_run` when the network refuses this computer's key as excluded: the invite is redeemed again
//! with a fresh key, and that key replaces `run.secret`.

use super::persisting_the_key::{run_with_file, Dir};
use super::*;
use crate::invite::test_responder::{head, key_frame, refusal_frame, Responder};
use crate::invite::wire::RedeemCode;
use crate::transport::net::facade::test_support::{secret, within};

#[tokio::test]
async fn redeem_for_run_writes_the_new_secret() {
    // given: a member that refuses the first request as excluded and hands out the key to the second
    let net = MemNet::new();
    let member = Responder::scripted(
        net.transport(&secret(2)),
        2,
        vec![refusal_frame(RedeemCode::Excluded), key_frame()],
    );
    let invite = ParsedInvite {
        head: head(),
        peers: vec![member.addr()],
        warnings: vec![],
    };
    let dir = Dir::new("rekey");
    let mut run = run_with_file(&dir);
    let before = run.secret.peer_id();
    // when
    within(
        "redeem_for_run",
        redeem_for_run_on(&invite, &mut run, 10, Some(&net)),
    )
    .await
    .unwrap();
    // then: the run holds a new key, and the settings file has the network key
    assert_ne!(run.secret.peer_id(), before, "the run must use a fresh key");
    assert!(run.effective_settings.net.network_key.is_some());
    assert_eq!(
        member.requests(),
        2,
        "the member must be asked exactly twice"
    );
}
