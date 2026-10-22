//! The topic's link table: one link per peer, the choice on simultaneous dials.
//! There is no reading of frames, messages or dials here.

use super::wire::{Frame, CLOSE_SHUTDOWN};
use crate::net::id::PeerId;
use crate::net::transport::PeerConn;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Whether to keep the new link instead of the current one. The new one wins if it was dialed by the same side as the current
/// (which it has already lost) or the dialers differ and the new one was dialed by the smaller `PeerId`.
pub fn keeps_new(me: PeerId, peer: PeerId, current_dialer: PeerId, new_dialer: PeerId) -> bool {
    new_dialer == current_dialer || new_dialer == me.min(peer)
}

pub struct Link {
    pub link_no: u64,
    pub dialer: PeerId,
    pub conn: Arc<dyn PeerConn>,
    pub out: mpsc::Sender<Frame>,
}

/// The result of `place`.
pub enum Placed {
    /// The first link with the peer.
    Up,
    /// The new one took the place of the old one (hand the old one over to be closed).
    Replaced(Link),
    /// The new one lost (hand it over to be closed).
    Rejected(Link),
}

pub struct Links {
    me: PeerId,
    by_peer: HashMap<PeerId, Link>,
}

impl Links {
    pub fn new(me: PeerId) -> Links {
        Links {
            me,
            by_peer: HashMap::new(),
        }
    }

    pub fn place(&mut self, peer: PeerId, link: Link) -> Placed {
        let Some(cur) = self.by_peer.get(&peer) else {
            self.by_peer.insert(peer, link);
            return Placed::Up;
        };
        if keeps_new(self.me, peer, cur.dialer, link.dialer) {
            let old = self.by_peer.insert(peer, link);
            return old.map_or(Placed::Up, Placed::Replaced);
        }
        Placed::Rejected(link)
    }

    /// Removes the link only if it is still the current one (the number matches); closes the removed one.
    pub fn remove_if(&mut self, peer: PeerId, link_no: u64) -> bool {
        let current = self.is_current(peer, link_no);
        let removed = current.then(|| self.by_peer.remove(&peer)).flatten();
        removed.iter().for_each(|l| l.conn.close(CLOSE_SHUTDOWN, b"gone"));
        current
    }

    /// The link `link_no` is the current one for this peer.
    pub fn is_current(&self, peer: PeerId, link_no: u64) -> bool {
        self.by_peer.get(&peer).is_some_and(|l| l.link_no == link_no)
    }

    /// Tests only: the number of links.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_peer.len()
    }

    pub fn contains(&self, peer: PeerId) -> bool {
        self.by_peer.contains_key(&peer)
    }

    /// Closes the link with the peer (if there is one); `Gone` will come from its reader.
    pub fn close(&self, peer: PeerId, code: u32, reason: &[u8]) {
        let link = self.by_peer.get(&peer);
        link.iter().for_each(|l| l.conn.close(code, reason));
    }

    /// The write queues of all peers except `except`.
    pub fn targets(&self, except: Option<PeerId>) -> Vec<(PeerId, mpsc::Sender<Frame>)> {
        let others = self.by_peer.iter().filter(|(p, _)| Some(**p) != except);
        others.map(|(p, l)| (*p, l.out.clone())).collect()
    }
}

impl Drop for Links {
    fn drop(&mut self) {
        let all = self.by_peer.values();
        all.for_each(|l| l.conn.close(CLOSE_SHUTDOWN, b"shutdown"));
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
