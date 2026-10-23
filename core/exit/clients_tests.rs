//! Tests for `exit/clients.rs`: the exit client registry.
//! Time is passed as the `active_peers_at(now)` parameter, without `sleep`: "more than `EXIT_CLIENT_LINGER` has passed" is a `now`
//! shifted forward. Lists are compared sorted: the registry promises no order, and a duplicate
//! member in the list is an error that the `Vec` comparison catches.

use super::*;
use std::time::{Duration, Instant};

use crate::transport::net::{NodeSecret, PeerId};

mod stream {
    use super::*;

    #[test]
    fn listed_after_stream_ok() {
        // given: member A has a connection
        let fx = setup();
        let conn = fx.register_conn(1);

        // when: a stream is opened on it
        let _stream = conn.conn().register_stream();

        // then: A is in the list
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[1]));
    }

    #[test]
    fn lingers_after_last_stream_closed() {
        // given: A has a single open stream
        let fx = setup();
        let conn = fx.register_conn(1);
        let stream = conn.conn().register_stream();

        // when: the stream is closed
        drop(stream);

        // then: right after closing A is still listed, and after EXIT_CLIENT_LINGER plus a second it is gone
        assert_eq!(
            fx.listed_after(Duration::ZERO),
            ids(&[1]),
            "a closed stream keeps the entry for EXIT_CLIENT_LINGER"
        );
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[]),
            "the entry should be gone after EXIT_CLIENT_LINGER"
        );
    }

    #[test]
    fn still_listed_just_before_linger_ends() {
        // given: A's single stream is closed
        let fx = setup();
        let conn = fx.register_conn(1);
        drop(conn.conn().register_stream());

        // when/then: one second before EXIT_CLIENT_LINGER ends A is still listed
        assert_eq!(fx.listed_after(EXIT_CLIENT_LINGER - SECOND), ids(&[1]));
    }

    #[test]
    fn repeated_streams_one_entry() {
        // given: three streams of one connection
        let fx = setup();
        let conn = fx.register_conn(1);
        let first = conn.conn().register_stream();
        let second = conn.conn().register_stream();
        let _third = conn.conn().register_stream();

        // then: A is in the list once
        assert_eq!(
            fx.listed_after(Duration::ZERO),
            ids(&[1]),
            "one entry per member"
        );

        // when: two of the three streams are closed
        drop(first);
        drop(second);

        // then: the third is still open — A is listed even after EXIT_CLIENT_LINGER
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[1]),
            "while at least one stream is open, the member is listed"
        );
    }

    #[test]
    fn two_clients_at_once() {
        // given: A and B each have a connection
        let fx = setup();
        let conn_a = fx.register_conn(1);
        let conn_b = fx.register_conn(2);

        // when: both have an open stream
        let _stream_a = conn_a.conn().register_stream();
        let _stream_b = conn_b.conn().register_stream();

        // then: both are listed
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[1, 2]));
    }

    #[test]
    fn empty_conn_is_not_client() {
        // given/when: the member's connection is created, no streams
        let fx = setup();
        let _conn = fx.register_conn(1);

        // then: it does not count as a client, now or later
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[]));
        assert_eq!(fx.listed_after(EXIT_CLIENT_LINGER + SECOND), ids(&[]));
    }
}

mod conn_close {
    use super::*;

    #[test]
    fn conn_drop_releases_open_streams() {
        // given: A has an open stream
        let fx = setup();
        let conn = fx.register_conn(1);
        let stream = conn.conn().register_stream();

        // when: the connection is closed while the stream guard is still alive (the splice tail)
        drop(conn);

        // then: after EXIT_CLIENT_LINGER A is gone, although the stream is formally not closed
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[]),
            "closing the connection forgets its open streams"
        );

        // when: the late stream guard is dropped
        drop(stream);

        // then: no panic and the entry did not come back to life (with shift ZERO a resurrected `last_active = now`
        // would be in the window; shift EXIT_CLIENT_LINGER + SECOND would purge it too)
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[]));
    }

    #[test]
    fn conn_drop_keeps_client_for_linger() {
        // given: A has an open stream
        let fx = setup();
        let conn = fx.register_conn(1);
        let _stream = conn.conn().register_stream();

        // when: the connection is closed
        drop(conn);

        // then: the EXIT_CLIENT_LINGER window runs from the moment of closing — A is still listed
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[1]));
    }

    #[test]
    fn second_conn_survives_first_close() {
        // given: A has two connections, each with an open stream
        let fx = setup();
        let first = fx.register_conn(1);
        let second = fx.register_conn(1);
        let _stream_first = first.conn().register_stream();
        let _stream_second = second.conn().register_stream();

        // when: the first connection is closed
        drop(first);

        // then: the second's stream keeps A listed even after EXIT_CLIENT_LINGER
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[1]),
            "per-connection accounting: a second live connection is not reset"
        );
    }

    #[test]
    fn stream_after_conn_close_is_noop() {
        // given: a reference to A's connection, which is already closed
        let fx = setup();
        let conn = fx.register_conn(1);
        let conn_ref = conn.conn();
        drop(conn);

        // when: "stream opened" arrives on the dead reference
        let _stream = conn_ref.register_stream();

        // then: A does not appear in the list
        assert_eq!(fx.listed_after(Duration::ZERO), ids(&[]));
        assert_eq!(fx.listed_after(EXIT_CLIENT_LINGER + SECOND), ids(&[]));
    }

    #[tokio::test]
    async fn abort_releases_stream() {
        // given: a task holds the guard of A's open stream and waits forever
        let fx = setup();
        let conn = fx.register_conn(1);
        let handle = fx.spawn_holding_stream(&conn).await;
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[1]),
            "while the task is alive, the stream is open"
        );

        // when: the task is aborted
        handle.abort();
        let _ = handle.await;

        // then: the guard is released — no open streams, A leaves after EXIT_CLIENT_LINGER
        assert_eq!(
            fx.listed_after(EXIT_CLIENT_LINGER + SECOND),
            ids(&[]),
            "the stream guard must fire on abort"
        );
    }
}

const SECOND: Duration = Duration::from_secs(1);

struct Fixture {
    clients: Clients,
}

fn setup() -> Fixture {
    Fixture {
        clients: Clients::default(),
    }
}

impl Fixture {
    fn register_conn(&self, seed: u8) -> ConnGuard {
        self.clients.register_conn(peer(seed))
    }

    /// Returns the list of clients at "now + `shift`", sorted for comparison.
    fn listed_after(&self, shift: Duration) -> Vec<PeerId> {
        sorted(self.clients.active_peers_at(Instant::now() + shift))
    }

    /// Spawns a task that takes the connection's stream guard and waits forever; returns
    /// once the guard has been taken.
    async fn spawn_holding_stream(&self, conn: &ConnGuard) -> tokio::task::JoinHandle<()> {
        let conn_ref = conn.conn();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let handle = crate::node::output::spawn(async move {
            let _stream = conn_ref.register_stream();
            let _ = ready_tx.send(());
            std::future::pending::<()>().await;
        });
        ready_rx.await.expect("the task should take the guard");
        handle
    }
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// A sorted list of members built from their seeds.
fn ids(seeds: &[u8]) -> Vec<PeerId> {
    sorted(seeds.iter().map(|s| peer(*s)).collect())
}

fn sorted(mut list: Vec<PeerId>) -> Vec<PeerId> {
    list.sort_by_key(|id| id.to_string());
    list
}
