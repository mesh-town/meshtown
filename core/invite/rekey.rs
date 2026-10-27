//! Joining by invite with a fallback key: when the issuer refuses this computer's key as excluded,
//! a fresh key is generated and the same invite is tried once more (a refusal as excluded does not use it up).
//!
//! What it does: runs the first attempt with the current key; on `Excluded` asks for a fresh key and runs the
//! second attempt, whose outcome is final. What it does not do: does not store keys and does not bind transports —
//! the caller's `attempt` does that.

use std::future::Future;

use crate::invite::redeem::RedeemError;
use crate::members::network_key::NetworkKey;
use crate::transport::net::NodeSecret;

/// The received network key and the new node key, if the join needed one.
pub struct Redeemed {
    pub key: NetworkKey,
    pub new_owner: Option<NodeSecret>,
}

/// Tries the invite with `owner`; if the issuer says `Excluded`, takes `fresh()` and tries once more.
/// Any other refusal of the first attempt is returned as is; a failed `fresh` returns its own error.
pub async fn redeem_or_rekey<F, Fut>(
    owner: &NodeSecret,
    fresh: impl FnOnce() -> Result<NodeSecret, RedeemError>,
    attempt: F,
) -> Result<Redeemed, RedeemError>
where
    F: Fn(NodeSecret) -> Fut,
    Fut: Future<Output = Result<NetworkKey, RedeemError>>,
{
    match attempt(owner.clone()).await {
        Ok(key) => Ok(Redeemed {
            key,
            new_owner: None,
        }),
        Err(RedeemError::Excluded) => second_try(fresh()?, attempt).await,
        Err(other) => Err(other),
    }
}

async fn second_try<F, Fut>(secret: NodeSecret, attempt: F) -> Result<Redeemed, RedeemError>
where
    F: Fn(NodeSecret) -> Fut,
    Fut: Future<Output = Result<NetworkKey, RedeemError>>,
{
    let key = attempt(secret.clone()).await?;
    Ok(Redeemed {
        key,
        new_owner: Some(secret),
    })
}

#[cfg(test)]
#[path = "rekey_tests.rs"]
mod tests;
