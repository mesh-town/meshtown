//! The advert of the own relay: a key is written as the record `relay/v1`, a vanished key retracts it, a new key
//! overwrites it, and nothing is written when there is nothing to retract.

use super::*;
use crate::node::node_state::own_value;
use crate::transport::net::contract::secret;
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const WAIT: Duration = Duration::from_secs(5);

mod advertise_relay {
    use super::*;

    #[tokio::test]
    async fn some_key_is_written() {
        // given
        let fx = setup();
        // when
        let (_keys, task) = fx.run(Some([4; 32]));
        // then
        fx.expect_advert(Some([4; 32])).await;
        task.abort();
    }

    #[tokio::test]
    async fn none_retracts() {
        // given: an old advert of this node lies in the replica
        let fx = setup();
        fx.write_advert([4; 32]).await;
        // when
        let (_keys, task) = fx.run(None);
        // then
        fx.expect_advert(None).await;
        task.abort();
    }

    #[tokio::test]
    async fn new_key_overwrites() {
        // given: the key is advertised
        let fx = setup();
        let (keys, task) = fx.run(Some([4; 32]));
        fx.expect_advert(Some([4; 32])).await;
        // when
        keys.send(Some([5; 32])).expect("the task listens");
        // then
        fx.expect_advert(Some([5; 32])).await;
        task.abort();
    }

    #[tokio::test]
    async fn key_that_vanishes_retracts_the_advert() {
        // given
        let fx = setup();
        let (keys, task) = fx.run(Some([4; 32]));
        fx.expect_advert(Some([4; 32])).await;
        // when
        keys.send(None).expect("the task listens");
        // then
        fx.expect_advert(None).await;
        task.abort();
    }

    #[tokio::test]
    async fn ends_when_the_sender_is_dropped() {
        // given
        let fx = setup();
        let (keys, task) = fx.run(Some([4; 32]));
        fx.expect_advert(Some([4; 32])).await;
        // when
        drop(keys);
        // then
        let ended = timeout(WAIT, task).await;
        assert!(ended.is_ok(), "the task outlived its sender");
    }
}

mod retract_relay {
    use super::*;

    #[tokio::test]
    async fn without_an_entry_writes_nothing() {
        // given: this node never advertised
        let fx = setup();
        // when
        retract_relay(&*fx.replica).await.expect("retract");
        // then: not even a tombstone
        let own = fx
            .replica
            .get(fx.replica.me(), RELAY_ADVERT_KEY)
            .await
            .expect("get");
        assert!(
            own.is_none(),
            "a tombstone was written for an advert that never existed"
        );
    }

    #[tokio::test]
    async fn after_a_tombstone_writes_nothing_more() {
        // given: the advert was retracted once
        let fx = setup();
        fx.write_advert([4; 32]).await;
        retract_relay(&*fx.replica).await.expect("first retract");
        let first = fx
            .replica
            .get(fx.replica.me(), RELAY_ADVERT_KEY)
            .await
            .expect("get");
        // when
        retract_relay(&*fx.replica).await.expect("second retract");
        // then
        let second = fx
            .replica
            .get(fx.replica.me(), RELAY_ADVERT_KEY)
            .await
            .expect("get");
        assert_eq!(first, second, "the second retract rewrote the tombstone");
    }

    #[tokio::test]
    async fn removes_a_live_entry() {
        // given
        let fx = setup();
        fx.write_advert([4; 32]).await;
        // when
        retract_relay(&*fx.replica).await.expect("retract");
        // then
        assert_eq!(fx.advert().await, None);
    }
}

// --- setup and helpers ---

struct Fixture {
    replica: Arc<MemReplica>,
}

/// An empty in-memory replica of the network, owned by this node.
fn setup() -> Fixture {
    let ctx = SyncCtx {
        secret: secret(1),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    Fixture {
        replica: backend.replica_mem(REPLICA_NET).expect("replica"),
    }
}

impl Fixture {
    /// Starts the task with the first key `first`; the sender is the handle to change the key.
    fn run(&self, first: Option<[u8; 32]>) -> (watch::Sender<Option<[u8; 32]>>, JoinHandle<()>) {
        let (tx, rx) = watch::channel(first);
        let task = crate::node::output::spawn(advertise_relay(rx, self.replica.clone()));
        (tx, task)
    }

    async fn write_advert(&self, key: [u8; 32]) {
        let value = Bytes::copy_from_slice(&key);
        self.replica
            .set_own(RELAY_ADVERT_KEY, value)
            .await
            .expect("write");
    }

    /// The own advert: the key in it, `None` when there is no live record.
    async fn advert(&self) -> Option<[u8; 32]> {
        let own = own_value(&*self.replica, RELAY_ADVERT_KEY)
            .await
            .expect("read");
        own.map(|(value, _)| value.as_ref().try_into().expect("a 32-byte key"))
    }

    /// Waits until the own advert is `want`.
    async fn expect_advert(&self, want: Option<[u8; 32]>) {
        let reached = timeout(WAIT, async {
            while self.advert().await != want {
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "the advert did not become {want:?}, it is {:?}",
            self.advert().await
        );
    }
}
