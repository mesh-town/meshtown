//! Helpers for the `block_addr` test: the host interface address, a delivery probe to it, waiting for the sink.
//!
//! What it does: `local_lan_ip` finds the address traffic leaves through to the outside; `lan_delivery_works`
//! checks that datagrams to this address reach the process at all (the macOS firewall decides by the
//! Mach-O UUID of the binary, for an unknown binary the packet vanishes silently); `wait_blocked` waits
//! until the `block_addr` sink (a separate task) has counted what it ate.
//! What it does not do: the address interception itself — that is in `shadow.rs`.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};

use super::LossyLink;

/// How long the delivery probe sends one datagram every `PROBE_STEP`.
const PROBE_TOTAL: Duration = Duration::from_secs(2);
const PROBE_STEP: Duration = Duration::from_millis(300);
/// The limit for waiting on the sink counter and the polling step.
const POLL_TOTAL: Duration = Duration::from_secs(2);
const POLL_STEP: Duration = Duration::from_millis(20);

/// The host interface address traffic leaves through to the outside; `None` — there is no network.
pub fn local_lan_ip() -> Option<IpAddr> {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect("192.0.2.1:9").ok()?;
    let ip = probe.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// Whether a datagram sent to `lan` reaches this process: a socket on `lan:0`, the client sends
/// it one every 300 ms for up to 2 s. `false` — the environment cuts LAN delivery (the test must be skipped).
pub async fn lan_delivery_works(lan: IpAddr) -> bool {
    let Ok(target) = UdpSocket::bind(SocketAddr::new(lan, 0)).await else {
        return false;
    };
    let Ok(client) = UdpSocket::bind("0.0.0.0:0").await else {
        return false;
    };
    let Ok(to) = target.local_addr() else {
        return false;
    };
    timeout(PROBE_TOTAL, ping_until_heard(&client, &target, to))
        .await
        .is_ok()
}

async fn ping_until_heard(client: &UdpSocket, target: &UdpSocket, to: SocketAddr) {
    let mut buf = [0u8; 8];
    let mut heard = false;
    while !heard {
        let _ = client.send_to(b"probe", to).await;
        // Mesh Town: the wait step is shorter than the overall deadline — a datagram eaten on the way is sent again
        heard = matches!(
            timeout(PROBE_STEP, target.recv_from(&mut buf)).await,
            Ok(Ok(_))
        );
    }
}

/// Waits up to 2 s in 20 ms steps for `stats().blocked` to grow to `want`; returns the last value.
pub async fn wait_blocked(link: &LossyLink, want: u64) -> u64 {
    let deadline = Instant::now() + POLL_TOTAL;
    while link.stats().blocked < want && Instant::now() < deadline {
        sleep(POLL_STEP).await;
    }
    link.stats().blocked
}
