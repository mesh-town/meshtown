//! Self-check of the `core/tests/lossy_link/` proxy: delay, loss share and bursts, the bottleneck (throughput and tail drop).
//!
//! The shared harness is `core/tests/common/lossy_link.rs`. There is no QUIC or iroh here — the congestion
//! measurement lives in `tests/congestion_lossy.rs`.

#[path = "common/lossy_link.rs"]
mod common;
mod lossy_link;

use std::time::Duration;

use common::{collect, gaps, packet, rig, spawn_collect, Arrival};
use lossy_link::LossModel;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delay_is_one_way_delay_in_both_directions() {
    let rig = rig(|c| c.one_way_delay = Duration::from_millis(60)).await;
    let idle = Duration::from_millis(300);
    let collector = spawn_collect(&rig.target, rig.epoch, idle);
    for seq in 0..5 {
        rig.send_c2s(seq, 200).await;
        sleep(Duration::from_millis(20)).await;
    }
    let up = collector.await.unwrap();
    assert_eq!(
        up.iter().map(|a| a.seq).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4],
        "order"
    );
    assert_delays_near_60(&up, "c2s");
    let collector = spawn_collect(&rig.client, rig.epoch, idle);
    for seq in 0..5 {
        let pkt = packet(seq, 200, rig.epoch);
        rig.target.send_to(&pkt, up[0].from).await.unwrap();
        sleep(Duration::from_millis(20)).await;
    }
    let down = collector.await.unwrap();
    assert_eq!(down.len(), 5);
    assert_delays_near_60(&down, "s2c");
}

fn assert_delays_near_60(got: &[Arrival], way: &str) {
    for a in got {
        assert!(
            a.delay >= Duration::from_millis(59),
            "{way} too fast: {:?}",
            a.delay
        );
    }
    // The upper bound is by the fastest packet: under machine load (load average 15–65)
    // a single packet is late by tens of milliseconds, while a doubled delay (120 ms) shows
    // in all packets at once.
    let fastest = got.iter().map(|a| a.delay).min().expect("no arrivals");
    assert!(
        fastest <= Duration::from_millis(75),
        "{way} too slow: the fastest {fastest:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn random_loss_share_is_near_configured() {
    let rig = rig(|c| c.loss = LossModel::Random { p: 0.03 }).await;
    let sent = 20_000u32;
    let (_, got) = tokio::join!(
        rig.burst_c2s(sent, 60),
        collect(&rig.target, rig.epoch, Duration::from_millis(400))
    );
    let s = rig.link.stats().c2s;
    assert_eq!(
        s.received, sent as u64,
        "the proxy did not accept everything: the socket buffer overflowed"
    );
    let share = s.lost_model as f64 / s.received as f64;
    assert!((0.025..=0.035).contains(&share), "loss share {share:.4}");
    assert_eq!(got.len() as u64, s.delivered);
    assert_eq!(s.delivered + s.lost_model, s.received);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn burst_loss_share_and_run_length_match_gilbert_elliott() {
    let rig = rig(|c| {
        c.loss = LossModel::Burst {
            mean_loss: 0.05,
            mean_burst: 10.0,
        }
    })
    .await;
    let sent = 20_000u32;
    let (_, got) = tokio::join!(
        rig.burst_c2s(sent, 60),
        collect(&rig.target, rig.epoch, Duration::from_millis(400))
    );
    assert_eq!(
        rig.link.stats().c2s.received,
        sent as u64,
        "the proxy did not accept everything"
    );
    let seqs: Vec<u32> = got.iter().map(|a| a.seq).collect();
    let runs = gaps(&seqs, sent);
    let share = (sent as usize - seqs.len()) as f64 / sent as f64;
    let mean_run = runs.iter().sum::<u32>() as f64 / runs.len() as f64;
    assert!((0.035..=0.065).contains(&share), "loss share {share:.4}");
    assert!(
        (6.0..=14.0).contains(&mean_run),
        "mean burst {mean_run:.2} of {}",
        runs.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bottleneck_passes_about_rate_and_tail_drops_excess() {
    let rig = rig(|c| {
        c.rate_mbit = Some(10.0);
        c.queue = Duration::from_millis(100);
    })
    .await;
    let collector = spawn_collect(&rig.target, rig.epoch, Duration::from_millis(400));
    let mut tick = tokio::time::interval(Duration::from_millis(1));
    for i in 0..1500u32 {
        tick.tick().await;
        rig.send_c2s(2 * i, 1200).await;
        rig.send_c2s(2 * i + 1, 1200).await;
    }
    let got = collector.await.unwrap();
    let (first, last) = (got.first().unwrap(), got.last().unwrap());
    let bytes: usize = got.iter().skip(1).map(|a| a.len).sum();
    let mbit = bytes as f64 * 8.0 / last.at.duration_since(first.at).as_secs_f64() / 1e6;
    assert!((8.5..=11.5).contains(&mbit), "throughput {mbit:.2} Mbit/s");
    assert!(
        got.windows(2).all(|w| w[0].seq < w[1].seq),
        "order violated"
    );
    let s = rig.link.stats().c2s;
    assert!(s.dropped_queue > 500, "the queue dropped nothing: {s:?}");
    assert_eq!(s.delivered + s.dropped_queue, s.received);
}
