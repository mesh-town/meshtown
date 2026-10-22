//! Admission of a `SYNC` connection: a tag from the network key, directions and topic; the handshake over a stream.
//! There is no peer or message state here.

use super::wire::{read_frame, write_frame, Frame, VERSION};
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::stream::StreamPair;
use blake3::Hash;

const CONTEXT: &str = "meshtown sync admission v1";

/// The admission key from the network seed.
pub fn admission_key(network_seed: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key(CONTEXT, network_seed)
}

/// The tag: from whom, to whom, on which topic; the direction is part of the tag.
pub fn tag(admission_key: &[u8; 32], from: PeerId, to: PeerId, topic: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_keyed(admission_key);
    h.update(from.as_bytes());
    h.update(to.as_bytes());
    h.update(topic);
    *h.finalize().as_bytes()
}

/// The accepting side's reply (the actor sends it after the link is established).
pub fn welcome(admission_key: &[u8; 32], me: PeerId, peer: PeerId, topic: &[u8; 32]) -> Frame {
    Frame::Welcome {
        tag: tag(admission_key, me, peer, topic),
    }
}

/// The dialer: sends `Hello`, waits for `Welcome`; a foreign tag or another frame is `Refused`.
pub async fn dial_handshake(
    pair: &mut StreamPair,
    admission_key: &[u8; 32],
    me: PeerId,
    peer: PeerId,
    topic: &[u8; 32],
) -> NetResult<()> {
    let hello = Frame::Hello {
        version: VERSION,
        topic: *topic,
        tag: tag(admission_key, me, peer, topic),
    };
    write_frame(&mut pair.send, &hello).await?;
    let want = tag(admission_key, peer, me, topic);
    match read_frame(&mut pair.recv).await? {
        Frame::Welcome { tag } if Hash::from(tag) == Hash::from(want) => Ok(()),
        _ => Err(NetError::Refused),
    }
}

/// The acceptor: reads `Hello`; a wrong version, tag or topic is `Refused` without a reply.
/// Returns the topic; does not send `Welcome`.
pub async fn accept_handshake(
    pair: &mut StreamPair,
    admission_key: &[u8; 32],
    me: PeerId,
    peer: PeerId,
    known: impl Fn(&[u8; 32]) -> bool,
) -> NetResult<[u8; 32]> {
    let Frame::Hello {
        version,
        topic,
        tag: got,
    } = read_frame(&mut pair.recv).await?
    else {
        return Err(NetError::Refused);
    };
    // tags are compared via `blake3::Hash`: its `==` runs in constant time
    let want = tag(admission_key, peer, me, &topic);
    let ok = version == VERSION && Hash::from(got) == Hash::from(want) && known(&topic);
    if ok {
        return Ok(topic);
    }
    Err(NetError::Refused)
}

#[cfg(test)]
#[path = "admit_tests.rs"]
mod tests;
