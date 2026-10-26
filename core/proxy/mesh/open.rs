//! Opening a mesh address for a proxy connection: the stream of a member's port, or an answer page.

use crate::node::output::short_peer_id;
use crate::ports_sharing::dial::DIAL_TIMEOUT;
use crate::ports_sharing::{dial_peer_port, PortReach};
use crate::proxy::front::{NetDirectory, NetEntry};
use crate::proxy::path::{open_chosen_route, OpenRefused, OpenedUpstream};
use crate::proxy::serve::ProxyConfig;
use crate::proxy::socks5::Target;

use super::catalog::{devices, lookup, rows_of, Device, Lookup};
use super::host::MeshHost;
use super::open_page::{ambiguous, not_found, Ask};

/// Opens a connection to the target: a `.meshtown` host is answered by the node itself and never reaches
/// route choice, an exit or DNS; any other target goes through `open_chosen_route`.
pub(in crate::proxy) async fn open_target(
    cfg: &ProxyConfig,
    nets: &NetDirectory,
    target: &Target,
    port: u16,
    tag: &'static str,
) -> Result<OpenedUpstream, OpenRefused> {
    if let Some(host) = MeshHost::parse(target) {
        return Ok(open_mesh(nets.entries_or(cfg), host, port).await);
    }
    open_chosen_route(cfg, target, port, tag).await
}

/// Opens `host`:`port` inside the mesh. Never fails: whatever cannot be reached is answered with a page.
async fn open_mesh(entries: Vec<NetEntry>, host: MeshHost, port: u16) -> OpenedUpstream {
    let list = devices(rows_of(&entries));
    match lookup(&list, &host) {
        Lookup::Found(device) => open_device(&entries, &list, device, &host, port).await,
        Lookup::Ambiguous(found) => {
            let kind = ambiguous(&list, found, port);
            Ask::bare(&entries, &host, port)
                .page("ambiguous", kind)
                .await
        }
        Lookup::NotFound => {
            let kind = not_found(&list, &host, port);
            Ask::bare(&entries, &host, port)
                .page("not-found", kind)
                .await
        }
    }
}

/// Dials the device's port in its network; a refusal becomes the page that explains it.
async fn open_device(
    entries: &[NetEntry],
    list: &[Device],
    device: Device,
    host: &MeshHost,
    port: u16,
) -> OpenedUpstream {
    let ask = Ask::of_device(entries, list, &device, host, port);
    let cfg = &ask.entry.cfg;
    let dialed = dial_peer_port(
        &cfg.stack,
        &ask.entry.ports,
        cfg.route.members(),
        device.peer,
        port,
        DIAL_TIMEOUT,
    )
    .await;
    match dialed {
        Ok(stream) => {
            let label = format!("mesh {}", short_peer_id(&device.peer));
            OpenedUpstream::stream(label, stream.send, stream.recv)
        }
        Err(error) => refused(&ask, error.reach).await,
    }
}

/// One page variant per way the dial can fail.
async fn refused(ask: &Ask<'_>, reach: PortReach) -> OpenedUpstream {
    match reach {
        PortReach::Offline => ask.offline().await,
        PortReach::NotOpened => ask.not_opened().await,
        PortReach::NoService | PortReach::Failed | PortReach::Unsupported => {
            ask.no_service(false).await
        }
        PortReach::TimedOut => ask.no_service(true).await,
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
