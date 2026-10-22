//! Accepting `REPLICA` service connections: the accepting side's handshake and handing the stream to the replica.
//! The `"domains"` replica is not accepted: its id is unknown to the service.

use super::host::ReplicaHost;
use super::replica::MeshReplica;
use crate::net::error::{NetError, NetResult};
use crate::net::stream::{BoxFuture, StreamPair};
use crate::net::transport::PeerConn;
use crate::net::ServiceHandler;
use crate::sync::flood::admit::{accept_handshake, welcome};
use crate::sync::flood::wire::{write_frame, CLOSE_REJECTED};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// The timeout for the stream and `Hello` of the accepting side.
const HANDSHAKE: Duration = Duration::from_secs(5);

pub(super) struct ReplicaService {
    host: Arc<ReplicaHost>,
}

impl ReplicaService {
    pub(super) fn new(host: Arc<ReplicaHost>) -> ReplicaService {
        ReplicaService { host }
    }
}

impl ServiceHandler for ReplicaService {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let host = self.host.clone();
        Box::pin(async move { serve(host, conn).await })
    }
}

async fn serve(host: Arc<ReplicaHost>, conn: Arc<dyn PeerConn>) {
    let Ok((pair, replica)) = admit(&host, &conn).await else {
        return conn.close(CLOSE_REJECTED, b"rejected");
    };
    replica.accept(conn.peer(), conn, pair).await;
}

/// The first stream and `Hello` within the timeout; the topic is the id of the replica being reconciled; a refusal, timeout or break is `Err`.
async fn admit(host: &ReplicaHost, conn: &Arc<dyn PeerConn>) -> NetResult<(StreamPair, Arc<MeshReplica>)> {
    let handshake = async {
        let wiring = &host.wiring;
        let mut pair = conn.accept().await?;
        let known = |t: &[u8; 32]| host.syncing(t).is_some();
        let topic = accept_handshake(&mut pair, &wiring.admission_key, wiring.me, conn.peer(), known).await?;
        let replica = host.syncing(&topic).ok_or(NetError::Refused)?;
        write_frame(&mut pair.send, &welcome(&wiring.admission_key, wiring.me, conn.peer(), &topic)).await?;
        Ok((pair, replica))
    };
    timeout(HANDSHAKE, handshake).await.unwrap_or(Err(NetError::Timeout))
}
