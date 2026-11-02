//! A model of one channel direction: loss, the bottleneck queue and delay.
//!
//! What it does: from a packet's arrival time and size decides whether it is lost by the model, dropped by the queue or
//! delivered at a given moment. What it does not have: sockets, tasks and counters — they are in `pipe.rs` and
//! `stats.rs`; the configuration of the whole proxy is in `mod.rs`.

use std::time::{Duration, Instant};

use rand::{RngExt, SeedableRng};
use rand_pcg::Pcg32;

/// The loss model; applied to both directions equally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LossModel {
    None,
    /// Independent losses with probability `p`.
    Random {
        p: f64,
    },
    /// Gilbert–Elliott: in the bad state all packets are lost; the mean loss share is `mean_loss`,
    /// the mean burst length is `mean_burst` packets.
    Burst {
        mean_loss: f64,
        mean_burst: f64,
    },
}

impl From<f64> for LossModel {
    /// Independent losses with probability `p`.
    fn from(p: f64) -> Self {
        LossModel::Random { p }
    }
}

impl From<[f64; 2]> for LossModel {
    /// Gilbert–Elliott losses from `[mean_loss, mean_burst]`.
    fn from([mean_loss, mean_burst]: [f64; 2]) -> Self {
        LossModel::Burst {
            mean_loss,
            mean_burst,
        }
    }
}

impl LossModel {
    /// Whether the next packet is lost; `bad` is the Gilbert–Elliott chain state (not touched
    /// for other models).
    pub fn lost(&self, bad: &mut bool, rng: &mut Pcg32) -> bool {
        match *self {
            LossModel::None => false,
            LossModel::Random { p } => rng.random_bool(p.clamp(0.0, 1.0)),
            LossModel::Burst {
                mean_loss,
                mean_burst,
            } => burst_step(bad, rng, mean_loss, mean_burst),
        }
    }
}

/// One chain step: first the state transition, then "in the bad state the packet is lost".
fn burst_step(bad: &mut bool, rng: &mut Pcg32, mean_loss: f64, mean_burst: f64) -> bool {
    let to_good = (1.0 / mean_burst).clamp(0.0, 1.0);
    let to_bad = (to_good * mean_loss / (1.0 - mean_loss)).clamp(0.0, 1.0);
    let switch = rng.random_bool(if *bad { to_good } else { to_bad });
    *bad ^= switch;
    *bad
}

/// The part of the proxy settings that shapes one direction.
#[derive(Debug, Clone)]
pub struct PathShape {
    /// One-way delay (after the queue).
    pub one_way_delay: Duration,
    pub loss: LossModel,
    /// The bottleneck speed, Mbit/s; `None` — no limit.
    pub rate_mbit: Option<f64>,
    /// The longest wait in the bottleneck queue; longer — a drop.
    pub queue: Duration,
}

/// What to do with a packet.
#[derive(Debug, Clone, Copy)]
pub enum Verdict {
    Lost,
    Dropped,
    Deliver(Instant),
}

/// The bottleneck: a queue with a constant output rate and tail drop.
struct Bottleneck {
    rate_mbit: Option<f64>,
    queue: Duration,
    next_free: Option<Instant>,
}

impl Bottleneck {
    /// The moment a packet leaves the bottleneck; `None` — the queue is longer than `queue`, the packet is dropped.
    fn pass(&mut self, now: Instant, size: usize) -> Option<Instant> {
        let Some(rate) = self.rate_mbit else {
            return Some(now);
        };
        let start = self.next_free.map_or(now, |free| free.max(now));
        if start.duration_since(now) > self.queue {
            return None;
        }
        let out = start + Duration::from_secs_f64(size as f64 * 8.0 / (rate * 1e6));
        self.next_free = Some(out);
        Some(out)
    }
}

/// The decider for one direction: loss, then the queue, then the delay.
pub struct Direction {
    loss: LossModel,
    bad: bool,
    pub(super) rng: Pcg32,
    bottleneck: Bottleneck,
    delay: Duration,
}

impl Direction {
    pub fn new(cfg: &PathShape, seed: u64) -> Direction {
        Direction {
            loss: cfg.loss,
            bad: false,
            rng: Pcg32::seed_from_u64(seed),
            bottleneck: Bottleneck {
                rate_mbit: cfg.rate_mbit,
                queue: cfg.queue,
                next_free: None,
            },
            delay: cfg.one_way_delay,
        }
    }

    pub fn admit(&mut self, now: Instant, size: usize) -> Verdict {
        if self.loss.lost(&mut self.bad, &mut self.rng) {
            return Verdict::Lost;
        }
        match self.bottleneck.pass(now, size) {
            Some(out) => Verdict::Deliver(out + self.delay),
            None => Verdict::Dropped,
        }
    }
}
