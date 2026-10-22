//! TCP socket tuning for the data path: one shared place for what is done on every connection.

use tokio::net::TcpStream;

/// Small exchanges (TLS handshake, HTTP/2 frames) do not wait for
/// a delayed acknowledgment, which saves ~40 ms.
pub fn disable_nagle(tcp: &TcpStream) {
    // The tuning is not required for operation: if it fails, the connection works as before.
    let _ = tcp.set_nodelay(true);
}
