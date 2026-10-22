//! The `mem` carrier: the `Hello`/`Accept`/`Refuse` handshake over a lossless path and the start of `MemConn`.
//! Streams and closing are in `conn`.

use crate::transport::net::carrier::{Carrier, CarrierRole};
use crate::transport::net::connect::{PacketPath, PathInfo};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::mem::conn::MemConn;
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::PeerConn;
use bytes::Bytes;
use std::io;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

type Started = (ServiceId, Arc<dyn PeerConn>);

pub struct MemCarrier {
    timeout: Duration,
}

impl MemCarrier {
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Its own handshake time limit (time is not paused in tests).
    pub fn with_timeout(timeout: Duration) -> Self {
        MemCarrier { timeout }
    }
}

impl Default for MemCarrier {
    fn default() -> Self {
        Self::new()
    }
}

fn net_other_error(msg: &str) -> NetError {
    NetError::Other(anyhow::anyhow!("{msg}"))
}

async fn first_frame(path: &dyn PacketPath) -> NetResult<Option<Frame>> {
    let pkt = match path.recv().await {
        Ok(pkt) => pkt,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    Frame::decode(pkt)
        .map(Some)
        .map_err(|e| net_other_error(&e.to_string()))
}

async fn handshake_as_dialer(path: Arc<dyn PacketPath>, svc: ServiceId) -> NetResult<Started> {
    path.send(Frame::Hello(Bytes::from_static(svc.as_bytes())).encode())
        .await?;
    match first_frame(path.as_ref()).await? {
        Some(Frame::Accept) => Ok((svc, MemConn::spawn(path, true))),
        Some(Frame::Refuse) | None => Err(NetError::Refused),
        Some(_) => Err(net_other_error("unexpected reply to Hello")),
    }
}

async fn handshake_as_acceptor(
    path: Arc<dyn PacketPath>,
    served_service_ids: Arc<[ServiceId]>,
) -> NetResult<Started> {
    let Some(Frame::Hello(name)) = first_frame(path.as_ref()).await? else {
        return Err(net_other_error("the first frame must be Hello"));
    };
    let Some(svc) = served_service_ids
        .iter()
        .find(|s| s.as_bytes() == name.as_ref())
        .copied()
    else {
        let _ = path.send(Frame::Refuse.encode()).await;
        return Err(NetError::Refused);
    };
    // The connection tasks go first: an early `Open` from the dialer would otherwise arrive with no reader.
    let conn = MemConn::spawn(path.clone(), false);
    path.send(Frame::Accept.encode()).await?;
    Ok((svc, conn))
}

async fn handshake(path: Arc<dyn PacketPath>, role: CarrierRole) -> NetResult<Started> {
    match role {
        CarrierRole::Dial(svc) => handshake_as_dialer(path, svc).await,
        CarrierRole::Accept(served_service_ids) => {
            handshake_as_acceptor(path, served_service_ids).await
        }
    }
}

async fn start_timed(
    timeout: Duration,
    path: Arc<dyn PacketPath>,
    role: CarrierRole,
) -> NetResult<Started> {
    let done = tokio::time::timeout(timeout, handshake(path.clone(), role)).await;
    let result = done.unwrap_or(Err(NetError::Timeout));
    if result.is_err() {
        path.close();
    }
    result
}

impl Carrier for MemCarrier {
    fn kind(&self) -> &'static str {
        "mem"
    }

    fn can_carry(&self, info: &PathInfo) -> bool {
        info.family == "mem"
    }

    fn start(
        &self,
        path: Box<dyn PacketPath>,
        role: CarrierRole,
    ) -> BoxFuture<'_, NetResult<Started>> {
        Box::pin(start_timed(self.timeout, Arc::from(path), role))
    }
}

#[cfg(test)]
#[path = "carrier_tests.rs"]
mod tests;
