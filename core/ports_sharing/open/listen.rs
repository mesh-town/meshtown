//! Binding the loopback port and the accepting task of one listener.

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use super::{lock, Listener, Slot};
use crate::node::output::say;

/// Binding `127.0.0.1:local`; an error is a ready text for `BindFailed`.
pub(super) fn bind_loopback(local: u16) -> Result<TcpListener, String> {
    if local == 0 {
        return Err("cannot open local port 0".into());
    }
    probe_wildcard(local)?;
    let busy = |e: std::io::Error| format!("cannot take 127.0.0.1:{local}: {e}");
    let std_l = std::net::TcpListener::bind(("127.0.0.1", local)).map_err(busy)?;
    std_l.set_nonblocking(true).map_err(busy)?;
    TcpListener::from_std(std_l).map_err(busy)
}

/// macOS lets you take `127.0.0.1:L` on top of somebody else's `0.0.0.0:L`/`[::]:L`; the probe catches such a conflicting listener.
#[cfg(target_os = "macos")]
fn probe_wildcard(local: u16) -> Result<(), String> {
    match std::net::TcpListener::bind(("0.0.0.0", local)) {
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            Err("port is in use by another program".into())
        }
        _ => Ok(()),
    }
}

#[cfg(not(target_os = "macos"))]
fn probe_wildcard(_local: u16) -> Result<(), String> {
    Ok(())
}

/// Listener: the task accepts through the cell and holds a `JoinSet` of connections.
pub(crate) fn spawn_listener<F, Fut>(listener: TcpListener, conn: F) -> Listener
where
    F: Fn(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let slot: Slot = Arc::new(Mutex::new(Some(listener)));
    let task = crate::node::output::spawn(accept_loop(slot.clone(), conn)).abort_handle();
    Listener { slot, task }
}

async fn accept_loop<F, Fut>(slot: Slot, conn: F)
where
    F: Fn(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut set = JoinSet::new();
    loop {
        let polled = std::future::poll_fn(|cx| match lock(&slot).as_ref() {
            None => Poll::Ready(None),
            Some(l) => l.poll_accept(cx).map(Some),
        });
        match polled.await {
            None => break,
            Some(Ok((tcp, _))) => {
                crate::node::output::spawn_in(&mut set, conn(tcp));
            }
            Some(Err(e)) => {
                say("port", format!("accepting a connection failed: {e}"));
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        while set.try_join_next().is_some() {}
    }
}
