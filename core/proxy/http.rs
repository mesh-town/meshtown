//! The HTTP "face": reads the rest of the request head, asks `path` for a route and answers the client with an HTTP code.

use std::time::Instant;

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::node::output::say;

use super::http_head::{
    parse_head, status_reply, HttpProxyRequest, HttpRequestKind, MAX_HEAD_BYTES,
};
use super::outcome::{outcome_line, ProxyOutcome};
use super::pac::{pac_proxy_list, pac_reply, pac_script, PacRequest, PAC_PATH};
use super::path;
use super::serve::{ProxyConfig, NO_PROXY_NETWORK};

/// Serves one HTTP connection: one request (`CONNECT`, an absolute URI or a PAC).
pub(super) async fn serve_http_connection(
    mut stream: TcpStream,
    cfg: Option<ProxyConfig>,
) -> Result<()> {
    let started = Instant::now();
    let Some((request_kind, buf)) = read_head(&mut stream).await? else {
        return Ok(());
    };
    match request_kind {
        HttpRequestKind::Proxy(request) => match cfg {
            Some(cfg) => forward(stream, &cfg, request, buf, started).await,
            None => refuse_no_network(stream, request, started).await,
        },
        HttpRequestKind::Pac(request) => serve_pac(stream, request).await,
    }
}

/// A proxy request while the proxy has no network: `502` and the journal line.
async fn refuse_no_network(
    mut stream: TcpStream,
    request: HttpProxyRequest,
    started: Instant,
) -> Result<()> {
    let outcome = ProxyOutcome::refused("no route", NO_PROXY_NETWORK);
    stream
        .write_all(&status_reply(502, &outcome.result))
        .await?;
    say(
        "http",
        outcome_line(
            &request.target.to_string(),
            &outcome,
            started.elapsed().as_millis(),
        ),
    );
    Ok(())
}

/// Proxying: `CONNECT` or a request with an absolute URI.
async fn forward(
    mut stream: TcpStream,
    cfg: &ProxyConfig,
    request: HttpProxyRequest,
    buf: Vec<u8>,
    started: Instant,
) -> Result<()> {
    let target_text = request.target.to_string();
    let outcome = match path::open_chosen_route(cfg, &request.target, request.port, "http").await {
        Ok(opened) => {
            stream.write_all(request.reply).await?;
            let mut prefix = request.upstream_head;
            prefix.extend_from_slice(&buf[request.head_len..]);
            path::relay_bytes(opened, &cfg.route, stream, &prefix).await
        }
        Err(refused) => {
            let code = if refused.timed_out { 504 } else { 502 };
            stream
                .write_all(&status_reply(code, &refused.outcome.result))
                .await?;
            refused.outcome
        }
    };
    say(
        "http",
        outcome_line(&target_text, &outcome, started.elapsed().as_millis()),
    );
    Ok(())
}

/// Serves the PAC; the port in the path line is the listener's actual port.
async fn serve_pac(mut stream: TcpStream, request: PacRequest) -> Result<()> {
    let proxy_list = pac_proxy_list(stream.local_addr()?, false);
    let script = pac_script(&proxy_list);
    stream
        .write_all(&pac_reply(&script, request.head_only))
        .await?;
    let method = if request.head_only { "HEAD" } else { "GET" };
    say(
        "http",
        format!("PAC served: {method} {PAC_PATH} → {proxy_list}"),
    );
    Ok(())
}

/// Reads until the head parses. `None` — the stream ended before the head or `400` has already been sent.
async fn read_head(stream: &mut TcpStream) -> Result<Option<(HttpRequestKind, Vec<u8>)>> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    loop {
        match parse_head(&buf) {
            Ok(Some(request_kind)) => return Ok(Some((request_kind, buf))),
            Ok(None) => {}
            Err(e) => {
                say("http", format!("request rejected: {e}"));
                stream.write_all(&status_reply(400, &e.to_string())).await?;
                return Ok(None);
            }
        }
        let mut chunk = [0u8; 4096];
        let want = chunk
            .len()
            .min(MAX_HEAD_BYTES.saturating_sub(buf.len()).max(1));
        let n = stream.read(&mut chunk[..want]).await?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}
