//! Tests only: a snapshot of the topic actor for checking links, dials and live tasks.

use super::Actor;
use tokio::sync::oneshot;

/// Live tasks are the reader and writer of each link plus the dials; an extra unaccepted link
/// shows up as an extra pair of tasks.
#[derive(Debug, PartialEq)]
pub struct Probed {
    pub links: usize,
    pub dials: usize,
    pub tasks: usize,
}

impl Actor {
    pub(super) fn probe(&self, reply: oneshot::Sender<Probed>) {
        let (links, dials, tasks) = (self.links.len(), self.dials, self.tasks.len());
        let _ = reply.send(Probed { links, dials, tasks });
    }
}
