//! Tests of the `mem` hub: node restart (part of `hub_tests.rs`).

use super::*;

#[tokio::test]
async fn takes_a_new_slot_in_the_same_net() {
    let r = setup_restart().await;

    let (old_net, old_slot) = net_and_slot(&r.old);
    let (new_net, new_slot) = net_and_slot(&r.new);

    assert_eq!(old_net, new_net, "a restart does not change the network");
    assert_ne!(old_slot, new_slot, "the slot is not reused");
}

#[tokio::test]
async fn the_old_connector_stops_listening() {
    let r = setup_restart().await;

    let got = timeout(Duration::from_secs(2), r.old.listen()).await;

    assert!(matches!(got, Ok(None)), "the old listen must end with None");
}

#[tokio::test]
async fn the_old_path_of_the_restarted_node_reads_eof() {
    let r = setup_restart().await;

    let err = recv_err(&*r.old_to_other).await;

    assert_eq!(err, ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn the_far_end_of_the_old_path_reads_eof() {
    let r = setup_restart().await;

    let err = recv_err(&*r.other_to_old).await;

    assert_eq!(err, ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn the_old_address_leads_nowhere() {
    let r = setup_restart().await;

    let err = dial_err(&r.other, &r.old_addr).await;

    assert!(
        matches!(err, NetError::NoRoute),
        "expected NoRoute, got {err:?}"
    );
}

#[tokio::test]
async fn the_new_address_is_dialable() {
    let r = setup_restart().await;

    let dialed = dial_ok(&r.other, &r.new.local_addr()).await;
    let arrived = listen_path(&r.new).await;

    dialed.send(pkt(5)).await.unwrap();
    assert_eq!(num(&recv_ok(&*arrived).await), 5);
}
