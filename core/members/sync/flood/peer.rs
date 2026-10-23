//! The tasks of one link: the reader (frames → the actor, silence and a break → `Gone`) and the writer
//! (queue → stream, `Ping` after `keepalive` without a write). There is no topic state here.

use super::topic::TopicCmd;
use super::tuning::Tuning;
use super::wire::{read_frame, write_frame, Frame, CLOSE_BAD_FRAME, CLOSE_IDLE, CLOSE_SHUTDOWN};
use crate::transport::net::error::NetError;
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::{BoxRecv, BoxSend, StreamPair};
use crate::transport::net::transport::PeerConn;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::timeout;

/// Starts the reader and writer of link `link_no` with peer `peer`; returns the write queue.
pub fn spawn_link(
    set: &mut JoinSet<()>,
    peer: PeerId,
    link_no: u64,
    conn: Arc<dyn PeerConn>,
    pair: StreamPair,
    cmd: mpsc::Sender<TopicCmd>,
    t: &Tuning,
) -> mpsc::Sender<Frame> {
    let (out, queue) = mpsc::channel(t.peer_write_queue_len);
    let StreamPair { send, recv } = pair;
    let read = read_loop(peer, link_no, conn, recv, cmd.clone(), t.idle);
    crate::transport::net::task_label::spawn_in(set, read);
    let write = write_loop(peer, link_no, send, queue, cmd, t.keepalive);
    crate::transport::net::task_label::spawn_in(set, write);
    out
}

async fn read_loop(
    peer: PeerId,
    link_no: u64,
    conn: Arc<dyn PeerConn>,
    mut recv: BoxRecv,
    cmd: mpsc::Sender<TopicCmd>,
    idle: Duration,
) {
    let reading = read_frames(peer, link_no, &mut recv, &cmd, idle);
    tokio::select! {
        code = reading => conn.close(code, b"read ended"),
        _ = conn.closed() => {}
    }
    let _ = cmd.send(TopicCmd::Gone { peer, link_no }).await;
}

/// Reads frames to the end; returns the code the link must be closed with.
async fn read_frames(
    peer: PeerId,
    link_no: u64,
    recv: &mut BoxRecv,
    cmd: &mpsc::Sender<TopicCmd>,
    idle: Duration,
) -> u32 {
    loop {
        match timeout(idle, read_frame(recv)).await {
            Err(_) => return CLOSE_IDLE,
            Ok(Err(NetError::Closed)) => return CLOSE_SHUTDOWN,
            Ok(Err(_)) => return CLOSE_BAD_FRAME,
            Ok(Ok(frame)) => {
                let sent = cmd
                    .send(TopicCmd::Frame {
                        peer,
                        link_no,
                        frame,
                    })
                    .await;
                if sent.is_err() {
                    return CLOSE_SHUTDOWN;
                }
            }
        }
    }
}

async fn write_loop(
    peer: PeerId,
    link_no: u64,
    mut send: BoxSend,
    mut queue: mpsc::Receiver<Frame>,
    cmd: mpsc::Sender<TopicCmd>,
    keepalive: Duration,
) {
    loop {
        let frame = match timeout(keepalive, queue.recv()).await {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(_) => Frame::Ping,
        };
        if write_frame(&mut send, &frame).await.is_err() {
            break;
        }
    }
    let _ = cmd.send(TopicCmd::Gone { peer, link_no }).await;
}
