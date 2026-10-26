//! The only file that names the framing crate (`townjump_pipe`) and its logger (`townjump_logger`).
//!
//! What it does: the opening side — a local UDP socket that gives every sender its own stream (`open_udp_listener`),
//! the crate's frame codec for the publishing loop, the crate log routed into the node journal.
//! What it does not do: it does not open streams itself (the caller's `connect` does) and does not serve a
//! published port (that is `serve`).

use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};

use townjump_logger::{Logger, SharedOut};
use townjump_pipe::{create_udp_framed_proxy, UdpProxy};

pub(super) use townjump_pipe::{encode_frame, FrameDecoder};

use super::log_bridge::JournalLines;
use super::session::UdpSession;

/// A listening local UDP port; dropping it closes the socket and every sender stream.
pub(crate) struct UdpListenerHandle {
    proxy: UdpProxy,
}

impl Drop for UdpListenerHandle {
    fn drop(&mut self) {
        self.proxy.task.abort();
    }
}

/// Crate log (WARN and ERROR) into the journal; the crate has no global logger.
fn port_logger() -> Logger {
    let out: SharedOut = Arc::new(Mutex::new(JournalLines::default()));
    Logger::new(Some(2.0), out.clone(), out)
}

/// Binds `127.0.0.1:<local>`; the first datagram of each new sender calls `connect` for its stream.
/// A bind error (`AddrInUse` included) is returned at once.
pub(crate) async fn open_udp_listener<F, Fut>(local: u16, connect: F) -> io::Result<UdpListenerHandle>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = io::Result<UdpSession>> + Send + 'static,
{
    let listen = ("127.0.0.1".to_string(), local);
    let proxy = create_udp_framed_proxy(listen, connect, port_logger()).await?;
    Ok(UdpListenerHandle { proxy })
}
