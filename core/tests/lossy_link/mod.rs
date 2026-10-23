//! A UDP proxy with loss, a bottleneck and delay — a long lossy path, inside the test process.
//!
//! What it does: listens on the front socket (`front_addr`), forwards the client's datagrams to `target` from
//! the back socket and the target's replies back to the client; each direction independently goes through the loss
//! model, the bottleneck queue and the delay (`model.rs`). One client, order is preserved.
//! What it does not do: system-level emulation (`tc`/`dnctl` do not work on macOS without sudo); the measurement on top of the
//! proxy is `tests/congestion_lossy.rs`, its self-check is `tests/lossy_link_{forwarding,shape,model}.rs`.
//! Files: `model.rs` — decisions, `pipe.rs` — the receive and delivery tasks, `stats.rs` — counters,
//! `shadow.rs` — binding a socket to an address held by someone else's `0.0.0.0` (`block_addr`), `lan.rs` —
//! helpers for the `block_addr` test (the LAN address, a delivery probe, waiting for the sink counter).
#![allow(dead_code)] // each test crate takes its own subset of the proxy

pub mod lan;
pub mod model;
mod pipe;
mod shadow;
pub mod stats;

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicUsize};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

pub use model::LossModel;

use model::Direction;
use pipe::{Client, Knobs, Peer, Target};
use stats::{Counters, LinkStats};

/// The proxy settings.
#[derive(Debug, Clone)]
pub struct LinkConfig {
    /// Where to forward the client's datagrams.
    pub target: SocketAddr,
    /// One-way delay (after the queue).
    pub one_way_delay: Duration,
    pub loss: LossModel,
    /// The bottleneck speed, Mbit/s; `None` — no limit.
    pub rate_mbit: Option<f64>,
    /// The longest wait in the bottleneck queue; longer — a drop.
    pub queue: Duration,
    pub seed: u64,
}

impl LinkConfig {
    /// A transparent channel: no delay, loss or speed limit.
    pub fn plain(target: SocketAddr) -> LinkConfig {
        LinkConfig {
            target,
            one_way_delay: Duration::ZERO,
            loss: LossModel::None,
            rate_mbit: None,
            queue: Duration::from_millis(100),
            seed: 1,
        }
    }
}

/// A running proxy; `drop` stops all tasks and frees the sockets.
pub struct LossyLink {
    front: SocketAddr,
    c2s: Arc<Counters>,
    s2c: Arc<Counters>,
    sink: Arc<Counters>,
    trap_c2s: Arc<AtomicUsize>,
    loss_c2s: Arc<AtomicU32>,
    tasks: Vec<JoinHandle<()>>,
}

impl LossyLink {
    /// Starts the proxy; call it inside a tokio runtime.
    pub fn start(cfg: LinkConfig) -> LossyLink {
        let front = Arc::new(bind_local());
        let back = Arc::new(bind_local());
        let client: Arc<dyn Peer> = Arc::new(Client::default());
        let target: Arc<dyn Peer> = Arc::new(Target(cfg.target));
        let mut link = LossyLink {
            front: front.local_addr().expect("the front socket's address"),
            c2s: Arc::default(),
            s2c: Arc::default(),
            sink: Arc::default(),
            trap_c2s: Arc::default(),
            loss_c2s: Arc::default(),
            tasks: Vec::new(),
        };
        let up = (front.clone(), client.clone(), back.clone(), target.clone());
        let knobs = Knobs {
            trap: link.trap_c2s.clone(),
            loss_milli: link.loss_c2s.clone(),
        };
        link.spawn_direction(&cfg, cfg.seed, up, link.c2s.clone(), knobs);
        let down = (back, target, front, client);
        link.spawn_direction(
            &cfg,
            cfg.seed ^ 0x9E37_79B9_7F4A_7C15,
            down,
            link.s2c.clone(),
            Knobs::default(),
        );
        link
    }

    /// The address the client knocks on.
    pub fn front_addr(&self) -> SocketAddr {
        self.front
    }

    /// Takes `addr` (`SO_REUSEADDR`, see `shadow.rs`) and silently eats everything that arrived
    /// on it: this is how an address is closed on which someone else's socket listens on `0.0.0.0`.
    pub fn block_addr(&mut self, addr: SocketAddr) -> io::Result<()> {
        let sock = UdpSocket::from_std(shadow::bind_shared(addr)?)?;
        self.tasks
            .push(meshtown_core::node::output::spawn(pipe::sink(
                sock,
                self.sink.clone(),
            )));
        Ok(())
    }

    pub fn stats(&self) -> LinkStats {
        let blocked = self.sink.snapshot().received;
        LinkStats {
            c2s: self.c2s.snapshot(),
            s2c: self.s2c.snapshot(),
            blocked,
        }
    }

    /// Starts a pair of direction tasks: (the receive socket and peer, the delivery socket and peer).
    fn spawn_direction(
        &mut self,
        cfg: &LinkConfig,
        seed: u64,
        ends: Ends,
        counters: Arc<Counters>,
        knobs: Knobs,
    ) {
        let (rx, from, tx, to) = ends;
        let (queue_tx, queue_rx) = unbounded_channel();
        let dir = Direction::new(cfg, seed);
        self.tasks
            .push(meshtown_core::node::output::spawn(pipe::ingest(
                rx,
                from,
                dir,
                counters.clone(),
                queue_tx,
                knobs,
            )));
        self.tasks
            .push(meshtown_core::node::output::spawn(pipe::deliver(
                tx, to, counters, queue_rx,
            )));
    }
}

type Ends = (Arc<UdpSocket>, Arc<dyn Peer>, Arc<UdpSocket>, Arc<dyn Peer>);

impl Drop for LossyLink {
    fn drop(&mut self) {
        self.tasks.iter().for_each(|t| t.abort());
    }
}

fn bind_local() -> UdpSocket {
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind the proxy socket");
    sock.set_nonblocking(true).expect("non-blocking mode");
    UdpSocket::from_std(sock).expect("the socket into the tokio runtime")
}
