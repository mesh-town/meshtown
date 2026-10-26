//! Binding the loopback port and the accepting task of one listener.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use super::{lock, Listener, OpenState, Reach, Shared, Slot, State};
use crate::exit::PeerConnPool;
use crate::node::output::say;
use crate::ports_sharing::dial::forward_to_peer_port;
use crate::ports_sharing::types::Opened;

/// Binds the TCP listener of one entry; on a failure the state says why and there is no listener.
pub(super) fn bind_tcp_entry(
    shared: &Arc<Shared>,
    o: Opened,
    pool: PeerConnPool,
    state: State,
    reach: Reach,
) -> Option<Listener> {
    match bind_loopback(o.local) {
        Ok(l) => {
            say("port", format!("127.0.0.1:{}: opened", o.local));
            Some(spawn_listener(l, forward_handler(shared, pool, o, reach)))
        }
        Err(text) => {
            say("port", format!("127.0.0.1:{}: not opened: {text}", o.local));
            *lock(&state) = OpenState::BindFailed(text);
            None
        }
    }
}

/// The handler of an accepted connection: forwards it and keeps the dial result in `reach`.
fn forward_handler(
    shared: &Shared,
    pool: PeerConnPool,
    target: Opened,
    reach: Reach,
) -> impl Fn(TcpStream) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static {
    let (stack, members) = (shared.stack.clone(), shared.members.clone());
    move |tcp| {
        let (stack, members, pool) = (stack.clone(), members.clone(), pool.clone());
        let (target, reach) = (target.clone(), reach.clone());
        Box::pin(async move {
            let dialed = reach.clone();
            let result = forward_to_peer_port(&stack, &pool, &members, &target, tcp, move || {
                *lock(&dialed) = None;
            })
            .await;
            if let Err(e) = result {
                *lock(&reach) = Some(e.reach);
            }
        })
    }
}

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

/// Probe before binding `127.0.0.1:local` over UDP: the same macOS conflict as for TCP, see `probe_wildcard`.
#[cfg(target_os = "macos")]
pub(super) fn probe_wildcard_udp(local: u16) -> Result<(), String> {
    match std::net::UdpSocket::bind(("0.0.0.0", local)) {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            Err("port is in use by another program".into())
        }
        _ => Ok(()),
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn probe_wildcard_udp(_local: u16) -> Result<(), String> {
    Ok(())
}

/// A ready text for `BindFailed` when the UDP socket cannot be taken.
pub(super) fn udp_bind_text(local: u16, e: &io::Error) -> String {
    format!("cannot take UDP 127.0.0.1:{local}: {e}")
}

/// Listener: the task accepts through the cell and holds a `JoinSet` of connections.
pub(crate) fn spawn_listener<F, Fut>(listener: TcpListener, conn: F) -> Listener
where
    F: Fn(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let slot: Slot = Arc::new(Mutex::new(Some(listener)));
    let task = crate::node::output::spawn(accept_loop(slot.clone(), conn)).abort_handle();
    Listener {
        slot: Some(slot),
        task,
    }
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
