//! The SOCKS5 server's I/O: the greeting, reading the request, the reply. The frames and their parsing are in
//! `super::frame`.

use std::net::SocketAddrV4;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::frame::{
    encode_socks5_reply, encode_socks5_reply_bound, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6,
    METHOD_NO_ACCEPTABLE, METHOD_NO_AUTH, VERSION,
};

/// Reads the client's greeting and replies "no authentication".
pub async fn read_greeting<S>(stream: &mut S) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.context("greeting")?;
    if head[0] != VERSION {
        bail!("this is not SOCKS5: version {}", head[0]);
    }
    let count = usize::from(head[1]);
    if count == 0 {
        bail!("the client sent no authentication methods");
    }
    let mut methods = vec![0u8; count];
    stream
        .read_exact(&mut methods)
        .await
        .context("list of authentication methods")?;
    if !methods.contains(&METHOD_NO_AUTH) {
        let _ = stream.write_all(&[VERSION, METHOD_NO_ACCEPTABLE]).await;
        bail!("the client cannot work without authentication");
    }
    stream
        .write_all(&[VERSION, METHOD_NO_AUTH])
        .await
        .context("reply to the greeting")?;
    Ok(())
}

/// Reads the request bytes — exactly as many as the frame has. Parsed by `super::frame::parse_socks5_request`.
/// An unknown address type is not read past four bytes: parsing will refuse it anyway.
pub async fn read_request_bytes<S>(stream: &mut S) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await.context("request")?;
    let mut raw = head.to_vec();
    match head[3] {
        ATYP_IPV4 => {
            let mut rest = [0u8; 6];
            stream
                .read_exact(&mut rest)
                .await
                .context("address and port")?;
            raw.extend_from_slice(&rest);
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await.context("name length")?;
            raw.push(len[0]);
            let mut rest = vec![0u8; usize::from(len[0]) + 2];
            stream
                .read_exact(&mut rest)
                .await
                .context("name and port")?;
            raw.extend_from_slice(&rest);
        }
        ATYP_IPV6 => {
            let mut rest = [0u8; 18];
            stream
                .read_exact(&mut rest)
                .await
                .context("address and port")?;
            raw.extend_from_slice(&rest);
        }
        _ => {}
    }
    Ok(raw)
}

/// Writes the server reply.
pub async fn write_reply<S>(stream: &mut S, code: u8) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream
        .write_all(&encode_socks5_reply(code))
        .await
        .context("SOCKS5 reply")?;
    stream.flush().await.context("SOCKS5 reply")?;
    Ok(())
}

/// Writes the server reply with the bound address and port (the answer to `UDP ASSOCIATE`).
pub async fn write_reply_bound<S>(stream: &mut S, code: u8, bound: SocketAddrV4) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream
        .write_all(&encode_socks5_reply_bound(code, bound))
        .await
        .context("SOCKS5 reply")?;
    stream.flush().await.context("SOCKS5 reply")?;
    Ok(())
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
