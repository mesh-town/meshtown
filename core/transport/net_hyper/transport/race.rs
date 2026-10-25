//! A race of dial attempts: the first success wins, the rest are dropped with the race.

use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::stream::BoxFuture;
use std::future::poll_fn;
use std::task::Poll;

type Attempt<'a, T> = Option<BoxFuture<'a, NetResult<T>>>;

/// Polls the live attempts once; a success ends the race, a failed one is dropped.
fn poll_round<T>(
    cx: &mut std::task::Context<'_>,
    direct: &mut [Attempt<'_, T>],
) -> Option<NetResult<T>> {
    for slot in direct.iter_mut() {
        let Some(fut) = slot.as_mut() else { continue };
        match fut.as_mut().poll(cx) {
            Poll::Ready(Ok(v)) => return Some(Ok(v)),
            Poll::Ready(Err(_)) => *slot = None,
            Poll::Pending => {}
        }
    }
    None
}

/// The first `Ok` of `key` and `direct`; when all fail, the error of `key`.
/// Dropping the result drops every attempt, so a lost one holds nothing.
pub(super) async fn first_ok<'a, T>(
    key: BoxFuture<'a, NetResult<T>>,
    direct: Vec<BoxFuture<'a, NetResult<T>>>,
) -> NetResult<T> {
    let mut key: Attempt<'a, T> = Some(key);
    let mut direct: Vec<Attempt<'a, T>> = direct.into_iter().map(Some).collect();
    let mut key_error: Option<NetError> = None;
    poll_fn(|cx| {
        if let Some(done) = poll_round(cx, &mut direct) {
            return Poll::Ready(done);
        }
        if let Some(fut) = key.as_mut() {
            match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(v)) => return Poll::Ready(Ok(v)),
                Poll::Ready(Err(e)) => {
                    key = None;
                    key_error = Some(e);
                }
                Poll::Pending => {}
            }
        }
        let alive = key.is_some() || direct.iter().any(Option::is_some);
        match alive {
            true => Poll::Pending,
            false => Poll::Ready(Err(key_error.take().unwrap_or(NetError::NoRoute))),
        }
    })
    .await
}

#[cfg(test)]
#[path = "race_tests.rs"]
mod tests;
