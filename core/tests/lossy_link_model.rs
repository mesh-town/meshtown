//! Self-check of the `core/tests/lossy_link/` proxy: the pure `Direction` model without sockets (determinism, burst statistics, the queue).
//!
//! The shared harness is `core/tests/common/lossy_link.rs`. There is no QUIC or iroh here — the congestion
//! measurement lives in `tests/congestion_lossy.rs`.

#[path = "common/lossy_link.rs"]
mod common;
mod lossy_link;

use std::time::{Duration, Instant};

use common::{gaps, model_cfg};
use lossy_link::model::{Direction, Verdict};
use lossy_link::LossModel;

#[test]
fn model_is_deterministic_per_seed() {
    let cfg = model_cfg(|c| c.loss = LossModel::Random { p: 0.3 });
    let (mut a, mut b, mut other) = (
        Direction::new(&cfg, 5),
        Direction::new(&cfg, 5),
        Direction::new(&cfg, 6),
    );
    let now = Instant::now();
    let lost = |d: &mut Direction| matches!(d.admit(now, 100), Verdict::Lost);
    let seq_a: Vec<bool> = (0..200).map(|_| lost(&mut a)).collect();
    let seq_b: Vec<bool> = (0..200).map(|_| lost(&mut b)).collect();
    let seq_o: Vec<bool> = (0..200).map(|_| lost(&mut other)).collect();
    assert_eq!(seq_a, seq_b);
    assert_ne!(seq_a, seq_o);
}

#[test]
fn model_burst_statistics_are_exact_enough_on_large_sample() {
    let cfg = model_cfg(|c| {
        c.loss = LossModel::Burst {
            mean_loss: 0.05,
            mean_burst: 10.0,
        }
    });
    let mut dir = Direction::new(&cfg, 3);
    let now = Instant::now();
    let total = 400_000u32;
    let lost_flags: Vec<bool> = (0..total)
        .map(|_| matches!(dir.admit(now, 100), Verdict::Lost))
        .collect();
    let share = lost_flags.iter().filter(|l| **l).count() as f64 / total as f64;
    assert!((0.045..=0.055).contains(&share), "share {share:.4}");
    let delivered: Vec<u32> = (0..total).filter(|i| !lost_flags[*i as usize]).collect();
    let runs = gaps(&delivered, total);
    let mean = runs.iter().sum::<u32>() as f64 / runs.len() as f64;
    assert!((9.0..=11.0).contains(&mean), "mean burst {mean:.2}");
}

#[test]
fn model_queue_drops_only_when_wait_exceeds_limit() {
    let cfg = model_cfg(|c| {
        c.rate_mbit = Some(8.0); // 1000 bytes take exactly 1 ms
        c.queue = Duration::from_micros(10_500);
        c.one_way_delay = Duration::from_millis(30);
    });
    let mut dir = Direction::new(&cfg, 1);
    let now = Instant::now();
    let verdicts: Vec<Verdict> = (0..20).map(|_| dir.admit(now, 1000)).collect();
    let accepted = verdicts
        .iter()
        .filter(|v| matches!(v, Verdict::Deliver(_)))
        .count();
    assert_eq!(
        accepted, 11,
        "waiting 0..10 ms is fine, 11 ms is already more than 10.5"
    );
    let Verdict::Deliver(second) = verdicts[1] else {
        panic!("the second packet must pass")
    };
    assert_eq!(
        second.duration_since(now),
        Duration::from_millis(32),
        "2 ms of queue + 30 ms of path"
    );
    let later = now + Duration::from_millis(500);
    assert!(
        matches!(dir.admit(later, 1000), Verdict::Deliver(_)),
        "the queue managed to drain"
    );
}
