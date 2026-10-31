//! One local gateway connection: its requests are rewritten onto the shared stream, its replies come from
//! the lane through a queue.

use std::sync::Arc;

use tokio::io::DuplexStream;
use tokio::io::{split, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::mpsc;

use super::frame::{read_raw, Kind, RawFrame};
use super::ids::LocalKey;
use super::lane::Lane;
use crate::exit::udpgw::codec::{parse_body, Frame};

/// Serves the connection until it ends. A connection that ends without closing its ids gets a close
/// for each of them, so the exit does not keep their sessions.
pub(super) async fn serve(
    lane: Arc<Lane>,
    key: LocalKey,
    end: DuplexStream,
    replies: mpsc::Receiver<Vec<u8>>,
) {
    let Some(own) = lane.reply_sender(key) else {
        return;
    };
    let (read, write) = split(end);
    tokio::select! {
        () = pump_requests(&lane, key, read, own) => {}
        () = pump_replies(write, replies) => {}
    }
    for shared in lane.unbind_local(key) {
        lane.queue_out(RawFrame::close(shared)).await;
    }
    lane.remove_local(key);
}

async fn pump_replies(mut write: WriteHalf<DuplexStream>, mut replies: mpsc::Receiver<Vec<u8>>) {
    while let Some(bytes) = replies.recv().await {
        if write.write_all(&bytes).await.is_err() {
            return;
        }
    }
}

async fn pump_requests(
    lane: &Lane,
    key: LocalKey,
    mut read: ReadHalf<DuplexStream>,
    own: mpsc::Sender<Vec<u8>>,
) {
    while let Ok(frame) = read_raw(&mut read).await {
        if !handle(lane, key, frame, &own).await {
            return;
        }
    }
}

/// Acts on one request; `false` ends the connection.
async fn handle(lane: &Lane, key: LocalKey, frame: RawFrame, own: &mpsc::Sender<Vec<u8>>) -> bool {
    let id = frame.conn_id();
    match frame.kind() {
        Kind::Keepalive => answer(own, &Frame::keepalive(id)),
        Kind::Other => answer(own, &Frame::error(id)),
        Kind::Close => close(lane, key, id).await,
        Kind::Data => data(lane, key, frame).await,
    }
}

/// A reply of the multiplexer itself; dropped if the connection does not keep up.
fn answer(own: &mpsc::Sender<Vec<u8>>, frame: &Frame) -> bool {
    let _ = own.try_send(RawFrame::of(frame).into_bytes());
    true
}

async fn close(lane: &Lane, key: LocalKey, local_id: u16) -> bool {
    let Some(shared) = lane.unbind(key, local_id) else {
        return true;
    };
    lane.queue_out(RawFrame::close(shared)).await
}

/// A datagram goes to the lane under its shared id. A body the exit could not parse would end the whole
/// shared stream there, so it ends only this connection here.
async fn data(lane: &Lane, key: LocalKey, frame: RawFrame) -> bool {
    if parse_body(frame.body()).is_err() {
        return false;
    }
    let (shared, swept) = lane.bind(key, frame.conn_id());
    for id in swept {
        lane.queue_out(RawFrame::close(id)).await;
    }
    let Some(shared) = shared else { return true };
    lane.queue_out(frame.with_conn_id(shared)).await
}
