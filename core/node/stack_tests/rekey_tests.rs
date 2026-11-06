use crate::invite::redeem::{redeem, RedeemError};
use crate::invite::rekey::redeem_or_rekey;
use crate::invite::test_responder::{
    head, ignore, key_frame, opts, own_key, refusal_frame, Responder,
};
use crate::invite::wire::RedeemCode;
use crate::transport::net::facade::test_support::{secret, within};
use crate::transport::net::{MemNet, NodeSecret};

mod over_the_wire {
    use super::*;

    #[tokio::test]
    async fn over_the_wire_the_second_round_uses_a_fresh_key() {
        // given: a member that refuses the first request as excluded and hands out the key to the second
        let net = MemNet::new();
        let member = Responder::scripted(
            net.transport(&secret(2)),
            2,
            vec![refusal_frame(RedeemCode::Excluded), key_frame()],
        );
        let peers = vec![member.addr()];
        let attempt = |owner: NodeSecret| {
            let (net, peers) = (net.clone(), peers.clone());
            async move {
                let t = crate::node::bind_ephemeral(Some(&net))
                    .await
                    .map_err(|e| RedeemError::Bind(e.to_string()))?;
                redeem(&head(), &peers, &opts(), &ignore, t, &owner).await
            }
        };
        let fresh = || Ok(secret(77));
        // when
        let got = within(
            "redeem_or_rekey",
            redeem_or_rekey(&secret(5), fresh, attempt),
        )
        .await;
        // then: the network key is parsed from the second answer and the new node key comes with it
        let done = got.expect("the second round is accepted");
        assert_eq!(done.key.to_bytes(), own_key().to_bytes());
        assert_eq!(
            done.new_owner.map(|s| s.peer_id()),
            Some(secret(77).peer_id())
        );
        assert_eq!(
            member.requests(),
            2,
            "the member must be asked exactly twice"
        );
    }
}
