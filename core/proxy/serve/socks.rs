//! One SOCKS5 connection: the greeting and the request head, the choice by command, the `CONNECT` transfer.

use std::time::Instant;

use anyhow::Result;
use tokio::net::TcpStream;

use super::{client_gone, ProxyConfig, NO_PROXY_NETWORK};
use crate::node::output::say;
use crate::proxy::front::NetDirectory;
use crate::proxy::outcome::{outcome_line, ProxyOutcome};
use crate::proxy::path::OpenedUpstream;
use crate::proxy::socks5;
use crate::proxy::{assoc, mesh, path};

/// Serves one SOCKS5 connection from the greeting to the end of the transfer. There is one route decision for
/// the whole connection and it does not depend on the target (a domain or an address);
/// a connection through an exit is closed if the exit is removed from `route.exits` while it is still open.
/// The greeting and the request bytes must arrive before `deadline`, otherwise the connection is closed silently.
pub(super) async fn serve_socks5_connection(
    mut stream: TcpStream,
    cfg: Option<ProxyConfig>,
    nets: NetDirectory,
    deadline: tokio::time::Instant,
) -> Result<()> {
    // One timestamp for the whole serving: the summary line prints it.
    let started = Instant::now();
    let head = tokio::time::timeout_at(deadline, read_request_head(&mut stream)).await;
    let Ok(raw) = head else {
        return Ok(());
    };
    let raw = raw?;
    let request = match socks5::parse_socks5_request(&raw) {
        Ok(request) => request,
        Err(e) => {
            // The request was not parsed — there is no target, nothing to write a summary line about.
            say("socks", format!("request rejected: {e}"));
            socks5::write_reply(&mut stream, e.reply_code()).await?;
            return Ok(());
        }
    };
    let Some(cfg) = cfg else {
        let target = match request.command {
            socks5::Command::Connect => request.target.to_string(),
            socks5::Command::UdpAssociate => "udp associate".to_string(),
        };
        return refuse_no_network(stream, &target, started).await;
    };
    dispatch_socks5_command(stream, cfg, nets, request, started).await
}

/// Hands a parsed request to the handler of its command.
async fn dispatch_socks5_command(
    stream: TcpStream,
    cfg: ProxyConfig,
    nets: NetDirectory,
    request: socks5::Request,
    started: Instant,
) -> Result<()> {
    match request.command {
        socks5::Command::Connect => serve_connect(stream, cfg, nets, request, started).await,
        socks5::Command::UdpAssociate => assoc::serve_associate(stream, cfg, started).await,
    }
}

/// The greeting (with its reply) and the raw bytes of the request.
async fn read_request_head(stream: &mut TcpStream) -> Result<Vec<u8>> {
    socks5::negotiate_auth_method(stream).await?;
    socks5::read_request_bytes(stream).await
}

/// The reply and the journal line for a request that arrived while the proxy has no network.
async fn refuse_no_network(mut stream: TcpStream, target: &str, started: Instant) -> Result<()> {
    socks5::write_reply(&mut stream, socks5::REP_HOST_UNREACHABLE).await?;
    let outcome = ProxyOutcome::refused("no route", NO_PROXY_NETWORK);
    say(
        "socks",
        outcome_line(target, &outcome, started.elapsed().as_millis()),
    );
    Ok(())
}

/// A `CONNECT` request: one route decision, the reply and the transfer to the end.
async fn serve_connect(
    mut stream: TcpStream,
    cfg: ProxyConfig,
    nets: NetDirectory,
    request: socks5::Request,
    started: Instant,
) -> Result<()> {
    let target_text = request.target.to_string();
    let opening = tokio::select! {
        opened = mesh::open::open_target(&cfg, &nets, &request.target, request.port, "socks") => Some(opened),
        () = client_gone(&stream) => None,
    };
    let outcome = match opening {
        None => ProxyOutcome::abandoned(),
        Some(Ok(opened)) => reply_and_relay(stream, opened, &cfg).await?,
        Some(Err(refused)) => {
            socks5::write_reply(&mut stream, socks5::REP_HOST_UNREACHABLE).await?;
            refused.outcome
        }
    };
    say(
        "socks",
        outcome_line(&target_text, &outcome, started.elapsed().as_millis()),
    );
    Ok(())
}

/// Tells the client the target is open and carries the bytes both ways to the end of the transfer.
async fn reply_and_relay(
    mut stream: TcpStream,
    opened: OpenedUpstream,
    cfg: &ProxyConfig,
) -> Result<ProxyOutcome> {
    socks5::write_reply(&mut stream, socks5::REP_OK).await?;
    Ok(path::relay_bytes(opened, &cfg.route, stream, &[]).await)
}
