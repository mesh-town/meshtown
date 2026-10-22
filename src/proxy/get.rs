//! The `get` console command: downloads a domain's home page through our own SOCKS5 server.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

use crate::http_min::{self, HttpLimits};
use crate::output::say;
use crate::socks5;

/// Downloads a domain's home page through our own SOCKS5 server (the `get` console command).
pub async fn get_via_socks(proxy: SocketAddr, domain: &str, user_agent: &str) -> Result<()> {
    let started = Instant::now();
    let tcp = socks5::client_connect(proxy, domain, 443).await?;
    let tls_cfg = Arc::new(http_min::tls_client_config()?);
    let server_name = ServerName::try_from(domain.to_string())
        .with_context(|| format!("{domain}: unusable server name for TLS"))?;
    let mut tls = TlsConnector::from(tls_cfg)
        .connect(server_name, tcp)
        .await
        .with_context(|| format!("{domain}: TLS handshake"))?;
    let request = http_min::HttpGetRequest {
        host: domain,
        path: "/",
        accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        user_agent,
    };
    let response = http_min::http_get(&mut tls, request, HttpLimits::default()).await?;
    say(
        "get",
        format!(
            "{domain}: code {}, {} bytes, {} ms",
            response.status,
            response.raw_bytes,
            started.elapsed().as_millis()
        ),
    );
    Ok(())
}
