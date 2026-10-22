//! The SOCKS5 client: used by the console command `get`, which goes through its own server.

use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::frame::{
    encode_socks5_request, Target, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, METHOD_NO_AUTH, REP_OK,
    VERSION,
};

/// The SOCKS5 client: negotiates with the proxy and hands over a stream ready for data transfer.
pub async fn client_connect(proxy: SocketAddr, domain: &str, port: u16) -> Result<TcpStream> {
    let mut stream = TcpStream::connect(proxy)
        .await
        .with_context(|| format!("proxy {proxy} is not responding"))?;
    stream
        .write_all(&[VERSION, 1, METHOD_NO_AUTH])
        .await
        .context("greeting to the proxy")?;
    let mut answer = [0u8; 2];
    stream
        .read_exact(&mut answer)
        .await
        .context("reply to the greeting")?;
    if answer[0] != VERSION || answer[1] != METHOD_NO_AUTH {
        bail!(
            "the proxy did not agree to work without authentication: {} {}",
            answer[0],
            answer[1]
        );
    }
    let request = encode_socks5_request(&Target::Domain(domain.to_string()), port)?;
    stream
        .write_all(&request)
        .await
        .context("connect request")?;
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await.context("proxy reply")?;
    if head[1] != REP_OK {
        bail!("the proxy refused, code {}", head[1]);
    }
    match head[3] {
        ATYP_IPV4 => {
            let mut rest = [0u8; 6];
            stream.read_exact(&mut rest).await.context("reply tail")?;
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await.context("reply tail")?;
            let mut rest = vec![0u8; usize::from(len[0]) + 2];
            stream.read_exact(&mut rest).await.context("reply tail")?;
        }
        ATYP_IPV6 => {
            let mut rest = [0u8; 18];
            stream.read_exact(&mut rest).await.context("reply tail")?;
        }
        other => bail!("the proxy sent an unknown address type: {other}"),
    }
    Ok(stream)
}
