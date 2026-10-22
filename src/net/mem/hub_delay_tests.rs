//! Tests of the `mem` hub: delivery delay (part of `hub_tests.rs`).

use super::*;

#[tokio::test]
async fn a_packet_is_not_delivered_before_the_delay() {
    let p = setup_pair().await;
    p.net.set_delay(Duration::from_millis(50));

    let started = Instant::now();
    p.a_to_b.send(pkt(1)).await.unwrap();
    recv_ok(&*p.b_to_a).await;

    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(50),
        "packet arrived in {took:?}, earlier than the delay"
    );
}

#[tokio::test]
async fn a_path_opened_after_set_delay_is_delayed_too() {
    let net = MemNet::new();
    net.set_delay(Duration::from_millis(50));
    let p = open_pair(&net).await;

    let started = Instant::now();
    p.a_to_b.send(pkt(1)).await.unwrap();
    recv_ok(&*p.b_to_a).await;

    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(50),
        "packet arrived in {took:?}, earlier than the delay"
    );
}

#[tokio::test]
async fn is_one_way_so_a_round_trip_takes_two() {
    let p = setup_pair().await;
    p.net.set_delay(Duration::from_millis(50));

    let started = Instant::now();
    p.a_to_b.send(pkt(1)).await.unwrap();
    recv_ok(&*p.b_to_a).await;
    p.b_to_a.send(pkt(2)).await.unwrap();
    recv_ok(&*p.a_to_b).await;

    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(100),
        "round trip in {took:?}, less than two delays"
    );
}

#[tokio::test]
async fn lifting_it_midway_through_100_packets_keeps_the_order() {
    let p = setup_pair().await;
    p.net.set_delay(Duration::from_millis(50));

    for i in 0..50 {
        p.a_to_b.send(pkt(i)).await.unwrap();
    }
    p.net.set_delay(Duration::ZERO);
    for i in 50..100 {
        p.a_to_b.send(pkt(i)).await.unwrap();
    }

    let mut got = Vec::new();
    for _ in 0..100 {
        got.push(num(&recv_ok(&*p.b_to_a).await));
    }
    assert_eq!(got, (0..100).collect::<Vec<u32>>());
}
