//! The end of a lookup that no longer matches the waiting `conn_id` is ignored.

use super::super::LIMITS;
use super::*;
use crate::exit::udpgw::test_support::cfg_with;
use crate::transport::net::facade::test_support::pid;
use std::collections::{HashMap, HashSet};

#[tokio::test]
async fn late_lookup_end_is_ignored() {
    let (replies, _queue) = mpsc::channel(1);
    let (results, _ended) = mpsc::channel(1);
    let mut gateway = Gateway {
        cfg: cfg_with(HashMap::new()),
        peer: pid(1),
        limits: LIMITS,
        replies,
        sessions: HashMap::new(),
        allowed: HashSet::new(),
        refused: HashSet::new(),
        pending: HashMap::new(),
        results,
        next_lookup: 2,
    };
    let frame = Frame {
        flags: crate::exit::udpgw::codec::FLAG_DATA,
        conn_id: 1,
        target: Some(Target::Domain("late.test".into(), 9)),
        data: b"a".to_vec(),
    };
    let task = crate::node::output::spawn(std::future::pending());
    let queue = VecDeque::from([frame]);
    gateway.pending.insert(1, Pending { id: 2, queue, task });
    let echo = SocketAddr::from(([127, 0, 0, 1], 9));
    let done = Resolved {
        conn_id: 1,
        id: 1,
        found: Some((echo, true)),
    };
    gateway.resolved(done).await;
    assert_eq!(gateway.pending[&1].id, 2);
    assert!(gateway.sessions.is_empty());
}
