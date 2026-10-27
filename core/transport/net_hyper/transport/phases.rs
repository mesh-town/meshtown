//! Two-phase dial: the direct attempt first, the attempt through a relay when it fails or runs out of time.

use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::stream::BoxFuture;
use std::time::Duration;
use tokio::time::timeout;

/// Which phase produced the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Direct,
    Relay,
}

/// Runs `attempt` under `limit`; a missed limit is `Timeout`.
async fn limited<T>(attempt: BoxFuture<'_, NetResult<T>>, limit: Duration) -> NetResult<T> {
    timeout(limit, attempt)
        .await
        .map_err(|_| NetError::Timeout)?
}

/// The direct phase under `direct_limit`; its success is returned and the relay phase is never polled.
/// `Closed` ends the dial. Any other failure starts the relay phase under `relay_limit`; when both fail,
/// the error of the direct phase is returned. Without a direct phase the relay error is returned.
pub(super) async fn direct_then_relay<'a, T>(
    direct: Option<BoxFuture<'a, NetResult<T>>>,
    direct_limit: Duration,
    relay: Option<BoxFuture<'a, NetResult<T>>>,
    relay_limit: Duration,
) -> NetResult<(T, Phase)> {
    let Some(direct) = direct else {
        return only_relay(relay, relay_limit).await;
    };
    let first = match limited(direct, direct_limit).await {
        Ok(done) => return Ok((done, Phase::Direct)),
        Err(NetError::Closed) => return Err(NetError::Closed),
        Err(first) => first,
    };
    let Some(relay) = relay else {
        return Err(first);
    };
    let second = limited(relay, relay_limit).await;
    second.map(|done| (done, Phase::Relay)).map_err(|_| first)
}

async fn only_relay<T>(
    relay: Option<BoxFuture<'_, NetResult<T>>>,
    relay_limit: Duration,
) -> NetResult<(T, Phase)> {
    let Some(relay) = relay else {
        return Err(NetError::NoRoute);
    };
    limited(relay, relay_limit)
        .await
        .map(|done| (done, Phase::Relay))
}

#[cfg(test)]
#[path = "phases_tests.rs"]
mod tests;
