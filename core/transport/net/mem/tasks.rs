//! Connection tasks of `mem`: the writer (frame queue → path) and the reader (path → frame parsing).
//! The reader does not wait for the user: data goes into the stream buffer.

use crate::transport::net::connect::PacketPath;
use crate::transport::net::mem::conn::CODE_PATH_LOST;
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::mem::shared::MemConnState;
use bytes::Bytes;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

/// Does not hold the reference to the value any further.
pub(super) async fn wait_until_closed(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|c| *c).await;
}

pub(super) async fn write_loop(shared: Arc<MemConnState>, mut rx: mpsc::UnboundedReceiver<Frame>) {
    let mut closed = shared.closed_tx.subscribe();
    loop {
        tokio::select! {
            biased;
            f = rx.recv() => match f {
                Some(f) => {
                    if !send_one(&shared, f).await {
                        break;
                    }
                }
                None => break,
            },
            _ = wait_until_closed(&mut closed) => break,
        }
    }
    while let Ok(f) = rx.try_recv() {
        if !send_one(&shared, f).await {
            break;
        }
    }
    shared.path.close();
}

async fn send_one(shared: &MemConnState, frame: Frame) -> bool {
    let Some(len) = send_encoded(&*shared.path, &frame).await else {
        return false;
    };
    shared.sent_bytes_total.fetch_add(len, Ordering::Relaxed);
    true
}

/// Encodes the frame and sends it; returns the number of bytes sent, `None` if the path refused.
async fn send_encoded(path: &dyn PacketPath, frame: &Frame) -> Option<u64> {
    let pkt = frame.encode();
    let len = pkt.len() as u64;
    path.send(pkt).await.ok()?;
    Some(len)
}

pub(super) async fn read_loop(shared: Arc<MemConnState>) {
    loop {
        let Ok(pkt) = shared.path.recv().await else {
            break;
        };
        shared
            .recv_bytes_total
            .fetch_add(pkt.len() as u64, Ordering::Relaxed);
        let Ok(frame) = Frame::decode(pkt) else {
            shared.violation("bad frame");
            return;
        };
        if !shared.dispatch(frame) {
            return;
        }
    }
    shared.close_remote(CODE_PATH_LOST, Bytes::new());
}
