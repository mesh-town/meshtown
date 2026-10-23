//! The topic actor: all peers, the duplicate cache and the dial queue belong to one task; commands arrive over mpsc.
//! The peer lifecycle: one link per peer, a deferred `NeighborDown`.

use super::admit::welcome;
use super::dial::dial_task;
use super::links::{Link, Links, Placed};
use super::peer::spawn_link;
use super::seen::{msg_id, Seen};
use super::tuning::Tuning;
use super::wire::{Frame, CLOSE_DUPLICATE, CLOSE_SLOW, MAX_BODY_BYTES};
use crate::transport::net::error::NetError;
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::StreamPair;
use crate::transport::net::transport::PeerConn;
use crate::transport::net::{SwarmEvent, WeakStack};
use bytes::Bytes;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
#[cfg(test)]
use tokio::sync::oneshot;
use tokio::sync::{broadcast, mpsc};
use tokio::task::{AbortHandle, JoinSet};

pub enum TopicCmd {
    Join(Vec<PeerId>),
    Broadcast(Bytes),
    Linked {
        peer: PeerId,
        dialer: PeerId,
        conn: Arc<dyn PeerConn>,
        pair: StreamPair,
    },
    Frame {
        peer: PeerId,
        link_no: u64,
        frame: Frame,
    },
    Gone {
        peer: PeerId,
        link_no: u64,
    },
    DialFailed {
        peer: PeerId,
        err: NetError,
    },
    /// Tests only: a snapshot of the actor.
    #[cfg(test)]
    Probe(oneshot::Sender<Probed>),
}

#[cfg(test)]
#[path = "topic_probe.rs"]
mod probe;
#[cfg(test)]
pub use probe::Probed;

/// The length of the actor's command channel. It is shared by the readers, writers and dials of all peers, so
/// the length does not depend on `Tuning::peer_write_queue_len` (the write queue for one peer); a full channel
/// slows the readers down rather than losing frames.
const CMD_QUEUE: usize = 256;

#[derive(Clone)]
pub struct TopicCtx {
    pub me: PeerId,
    pub topic: [u8; 32],
    pub admission_key: [u8; 32],
    pub stack: Arc<OnceLock<WeakStack>>,
    pub tuning: Tuning,
}

/// Starts the topic actor: the command channel, the event channel and the stop handle.
pub fn spawn_topic(
    ctx: TopicCtx,
) -> (
    mpsc::Sender<TopicCmd>,
    broadcast::Sender<SwarmEvent>,
    AbortHandle,
) {
    let (tx, rx) = mpsc::channel(CMD_QUEUE);
    let (events, _) = broadcast::channel(ctx.tuning.event_channel_len);
    let seen = Seen::new(ctx.tuning.seen_cap, ctx.tuning.seen_life);
    let actor = Actor {
        links: Links::new(ctx.me),
        seen,
        dialing: HashSet::new(),
        deferred: HashSet::new(),
        next_link_no: 0,
        tasks: JoinSet::new(),
        #[cfg(test)]
        dials: 0,
        tx: tx.clone(),
        events: events.clone(),
        ctx,
    };
    let task = crate::transport::net::task_label::spawn(actor.run(rx));
    (tx, events, task.abort_handle())
}

struct Actor {
    ctx: TopicCtx,
    links: Links,
    seen: Seen,
    /// Peers we are dialing right now.
    dialing: HashSet<PeerId>,
    /// The current link with the peer ended while our dial was in progress: `NeighborDown` waits for its outcome.
    deferred: HashSet<PeerId>,
    next_link_no: u64,
    tasks: JoinSet<()>,
    /// Tests only: how many dials were started.
    #[cfg(test)]
    dials: usize,
    tx: mpsc::Sender<TopicCmd>,
    events: broadcast::Sender<SwarmEvent>,
}

impl Actor {
    async fn run(mut self, mut rx: mpsc::Receiver<TopicCmd>) {
        loop {
            let cmd = tokio::select! {
                Some(cmd) = rx.recv() => cmd,
                Some(_) = self.tasks.join_next() => continue,
                else => return,
            };
            if !self.dispatch_topic_cmd(cmd) {
                return;
            }
        }
    }

    /// `false` — the actor ends.
    fn dispatch_topic_cmd(&mut self, cmd: TopicCmd) -> bool {
        match cmd {
            TopicCmd::Join(peers) => peers.into_iter().for_each(|p| self.dial(p)),
            TopicCmd::Broadcast(body) => self.broadcast(body),
            TopicCmd::Linked {
                peer,
                dialer,
                conn,
                pair,
            } => self.linked(peer, dialer, conn, pair),
            TopicCmd::Frame {
                peer,
                link_no,
                frame,
            } => self.frame(peer, link_no, frame),
            TopicCmd::Gone { peer, link_no } => self.gone(peer, link_no),
            TopicCmd::DialFailed { peer, err } => return self.dial_failed(peer, err),
            #[cfg(test)]
            TopicCmd::Probe(reply) => self.probe(reply),
        }
        true
    }

    fn emit(&self, event: SwarmEvent) {
        let _ = self.events.send(event);
    }

    fn dial(&mut self, peer: PeerId) {
        let skip = peer == self.ctx.me || self.links.contains(peer) || self.dialing.contains(&peer);
        if skip {
            return;
        }
        self.dialing.insert(peer);
        #[cfg(test)]
        {
            self.dials += 1;
        }
        crate::transport::net::task_label::spawn_in(
            &mut self.tasks,
            dial_task(self.ctx.clone(), peer, self.tx.clone()),
        );
    }

    fn broadcast(&mut self, body: Bytes) {
        self.seen.remember_if_new(msg_id(&body), Instant::now());
        let ttl = self.ctx.tuning.ttl_hops;
        self.send_all(
            None,
            Frame::Msg {
                ttl,
                body: body.to_vec(),
            },
        );
    }

    /// Puts the frame in the queues of all peers except `except`; a full queue means the peer is too slow.
    fn send_all(&mut self, except: Option<PeerId>, frame: Frame) {
        for (peer, out) in self.links.targets(except) {
            if out.try_send(frame.clone()).is_err() {
                self.links.close(peer, CLOSE_SLOW, b"slow");
            }
        }
    }

    fn frame(&mut self, peer: PeerId, link_no: u64, frame: Frame) {
        if !self.links.is_current(peer, link_no) {
            return;
        }
        let Frame::Msg { ttl, body } = frame else {
            return;
        };
        self.incoming(peer, ttl, body);
    }

    fn incoming(&mut self, peer: PeerId, ttl: u8, body: Vec<u8>) {
        // we do not trust the ttl from the wire: we do not forward beyond our own limit
        let ttl = ttl.min(self.ctx.tuning.ttl_hops);
        let fresh = body.len() <= MAX_BODY_BYTES
            && self.seen.remember_if_new(msg_id(&body), Instant::now());
        if !fresh {
            return;
        }
        let msg = Bytes::from(body.clone());
        self.emit(SwarmEvent::Received { from: peer, msg });
        if ttl > 1 {
            self.send_all(Some(peer), Frame::Msg { ttl: ttl - 1, body });
        }
    }

    fn linked(&mut self, peer: PeerId, dialer: PeerId, conn: Arc<dyn PeerConn>, pair: StreamPair) {
        self.next_link_no += 1;
        let link_no = self.next_link_no;
        let cmd = self.tx.clone();
        let out = spawn_link(
            &mut self.tasks,
            peer,
            link_no,
            conn.clone(),
            pair,
            cmd,
            &self.ctx.tuning,
        );
        self.finish_dial(peer, dialer);
        let link = Link {
            link_no,
            dialer,
            conn,
            out: out.clone(),
        };
        let placed = self.links.place(peer, link);
        self.settle(peer, dialer, out, placed);
    }

    /// Our dial to the peer reached a link: it is no longer "in progress".
    fn finish_dial(&mut self, peer: PeerId, dialer: PeerId) {
        if dialer != self.ctx.me {
            return;
        }
        self.dialing.remove(&peer);
    }

    /// The outcome of `place`: close the rejected link, otherwise greet and announce the peer.
    fn settle(&mut self, peer: PeerId, dialer: PeerId, out: mpsc::Sender<Frame>, placed: Placed) {
        if let Placed::Rejected(lost) = placed {
            return lost.conn.close(CLOSE_DUPLICATE, b"duplicate");
        }
        self.greet(peer, dialer, &out);
        self.announce(peer, placed);
    }

    /// The side that accepted the link replies with a `Welcome` first frame; the dialer waits for it.
    fn greet(&self, peer: PeerId, dialer: PeerId, out: &mpsc::Sender<Frame>) {
        if dialer != peer {
            return;
        }
        let first = welcome(&self.ctx.admission_key, self.ctx.me, peer, &self.ctx.topic);
        let _ = out.try_send(first);
    }

    /// Close the old link, `NeighborUp` only on the first link.
    fn announce(&mut self, peer: PeerId, placed: Placed) {
        if let Placed::Replaced(old) = placed {
            return old.conn.close(CLOSE_DUPLICATE, b"replaced");
        }
        if self.deferred.remove(&peer) {
            return;
        }
        self.emit(SwarmEvent::NeighborUp(peer));
    }

    fn gone(&mut self, peer: PeerId, link_no: u64) {
        if !self.links.remove_if(peer, link_no) {
            return;
        }
        if self.dialing.contains(&peer) {
            self.deferred.insert(peer);
            return;
        }
        self.emit(SwarmEvent::NeighborDown(peer));
    }

    /// `false` — the stack is closed, the topic ends.
    fn dial_failed(&mut self, peer: PeerId, err: NetError) -> bool {
        tracing::debug!(%peer, "dial into the topic failed: {err}");
        self.dialing.remove(&peer);
        if matches!(err, NetError::Closed) {
            self.emit(SwarmEvent::Closed("stack closed".into()));
            return false;
        }
        self.release_deferred(peer);
        true
    }

    /// The deferred `NeighborDown` is emitted if there is still no link with the peer.
    fn release_deferred(&mut self, peer: PeerId) {
        let owed = self.deferred.remove(&peer);
        if !owed || self.links.contains(peer) {
            return;
        }
        self.emit(SwarmEvent::NeighborDown(peer));
    }
}
