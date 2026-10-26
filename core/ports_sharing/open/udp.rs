//! The UDP listener of one opened port: a task that owns the socket and opens a stream per sender.
//!
//! What it does: takes `127.0.0.1:<local>` over UDP (retrying a busy port for a short time, since the
//! previous listener of the same port is closed asynchronously), dials the publisher once for every new
//! sender and keeps the result of the last dial in the entry.
//! What it does not do: it does not frame datagrams (that is `udp::pipe`) and does not decide who is a member.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use super::listen::{probe_wildcard_udp, udp_bind_text};
use super::{lock, Listener, OpenState, Reach, Shared, State};
use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::node::output::say;
use crate::ports_sharing::dial::{dial_peer, PortDialError, DIAL_TIMEOUT};
use crate::ports_sharing::frame::PortRequest;
use crate::ports_sharing::types::{Opened, Proto};
use crate::ports_sharing::udp::{
    open_udp_listener, UdpListenerHandle, UdpSession, MAX_SENDERS_PER_OPEN, OPEN_IDLE,
    UDP_SESSION_MARK,
};
use crate::transport::net::NetStack;

/// How long a busy local port is retried: the previous proxy of the same port may still be closing.
const UDP_REBIND_WAIT: Duration = Duration::from_secs(1);
const REBIND_STEP: Duration = Duration::from_millis(50);
/// A journal line about a failing sender or dial is written no more often than this.
const LOG_EVERY: Duration = Duration::from_secs(10);

type SessionFuture = Pin<Box<dyn Future<Output = io::Result<UdpSession>> + Send>>;

/// Starts the UDP listener task. The entry is `Listening` until the task reports a bind failure.
pub(super) fn bind_udp_entry(
    shared: &Arc<Shared>,
    o: Opened,
    pool: PeerConnPool,
    state: State,
    reach: Reach,
) -> Option<Listener> {
    let label = format!("{} {}/udp", shared.members.peer_label(o.node), o.port);
    let local = o.local;
    let dialer = Arc::new(Dialer {
        stack: shared.stack.clone(),
        members: shared.members.clone(),
        pool,
        target: o,
        label: label.clone(),
        reach,
        permits: Arc::new(Semaphore::new(MAX_SENDERS_PER_OPEN)),
        last_log: Mutex::new(None),
    });
    let connect = move || -> SessionFuture {
        let dialer = dialer.clone();
        Box::pin(async move { dialer.session().await })
    };
    let task = crate::node::output::spawn(hold_listener(local, label, state, connect));
    Some(Listener {
        slot: None,
        task: task.abort_handle(),
    })
}

/// Opens the socket and holds it until the task is aborted; a final failure goes to the state.
async fn hold_listener<F>(local: u16, label: String, state: State, connect: F)
where
    F: Fn() -> SessionFuture + Clone + Send + Sync + 'static,
{
    match open_with_retry(local, connect).await {
        Ok(handle) => {
            say("port", format!("127.0.0.1:{local}/udp: opened"));
            let _held: UdpListenerHandle = handle;
            std::future::pending::<()>().await
        }
        Err(text) => {
            say("port", format!("{label} not opened: {text}"));
            *lock(&state) = OpenState::BindFailed(text);
        }
    }
}

/// The probe and the bind are repeated together while the port is busy, up to `UDP_REBIND_WAIT`.
async fn open_with_retry<F>(local: u16, connect: F) -> Result<UdpListenerHandle, String>
where
    F: Fn() -> SessionFuture + Clone + Send + Sync + 'static,
{
    if local == 0 {
        return Err("cannot open local port 0".into());
    }
    let started = Instant::now();
    loop {
        let (text, busy) = match try_open(local, connect.clone()).await {
            Ok(handle) => return Ok(handle),
            Err(failed) => failed,
        };
        if !busy || started.elapsed() >= UDP_REBIND_WAIT {
            return Err(text);
        }
        tokio::time::sleep(REBIND_STEP).await;
    }
}

/// One attempt: the error is the text and whether the port is merely busy (worth a retry).
async fn try_open<F>(local: u16, connect: F) -> Result<UdpListenerHandle, (String, bool)>
where
    F: Fn() -> SessionFuture + Clone + Send + Sync + 'static,
{
    probe_wildcard_udp(local).map_err(|text| (text, true))?;
    open_udp_listener(local, connect)
        .await
        .map_err(|e| (udp_bind_text(local, &e), e.kind() == io::ErrorKind::AddrInUse))
}

/// What a new sender needs to get its stream; shared by all senders of the listener.
struct Dialer {
    stack: NetStack,
    members: Members,
    pool: PeerConnPool,
    target: Opened,
    label: String,
    reach: Reach,
    permits: Arc<Semaphore>,
    last_log: Mutex<Option<Instant>>,
}

impl Dialer {
    /// A stream for a new sender: a permit first, then the dial. Every failure drops the datagram.
    async fn session(&self) -> io::Result<UdpSession> {
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| self.too_many())?;
        let req = PortRequest {
            proto: Proto::Udp,
            port: self.target.port,
        };
        let dialed = dial_peer(
            &self.stack,
            &self.pool,
            &self.members,
            self.target.node,
            req,
            DIAL_TIMEOUT,
        )
        .await;
        let stream = dialed.map_err(|e| self.dial_failed(e))?;
        *lock(&self.reach) = None;
        Ok(UdpSession::new(
            stream.send,
            stream.recv,
            OPEN_IDLE,
            Some(permit),
        ))
    }

    fn too_many(&self) -> io::Error {
        let limit = MAX_SENDERS_PER_OPEN;
        self.log_throttled(format!("too many senders (limit {limit}), datagram dropped"));
        refused("too many senders")
    }

    fn dial_failed(&self, e: PortDialError) -> io::Error {
        *lock(&self.reach) = Some(e.reach);
        self.log_throttled(e.text.clone());
        refused(&e.text)
    }

    fn log_throttled(&self, text: String) {
        let mut last = lock(&self.last_log);
        if last.is_some_and(|t| t.elapsed() < LOG_EVERY) {
            return;
        }
        *last = Some(Instant::now());
        say("port", format!("{}: {text}", self.label));
    }
}

/// The error the crate sees: the mark keeps it out of the journal bridge, we write our own line.
fn refused(text: &str) -> io::Error {
    io::Error::other(format!("{UDP_SESSION_MARK}{text}"))
}
