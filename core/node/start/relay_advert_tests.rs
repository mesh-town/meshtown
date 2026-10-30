//! The advert of the own relay: the relay (key and address) is written as the record `relay/v1`, a vanished relay
//! retracts it, a new one overwrites it, and nothing is written when there is nothing to retract.

use super::*;
use crate::node::start::relay_candidates::decode_relay_value;
use bytes::Bytes;
use std::net::{Ipv4Addr, SocketAddrV4};
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
    async fn some_advert_is_written() {
        // given
        let fx = setup();
        // when
        let (_adverts, task) = fx.run(Some(advert(4)));
        // then
        fx.expect_advert(Some(advert(4))).await;
        task.abort();
    }

    #[tokio::test]
    async fn written_advert_carries_the_address() {
        // given
        let fx = setup();
        let wanted = advert(4);
        // when
        let (_adverts, task) = fx.run(Some(wanted));
        // then: the key, the IPv4 address and the big-endian UDP port
        fx.expect_advert(Some(wanted)).await;
        let value = fx.raw_advert().await.expect("a record");
        assert_eq!(value.len(), 38, "the record has an unexpected length");
        assert_eq!(&value[..32], &wanted.key);
        assert_eq!(&value[32..36], &wanted.address.ip().octets());
        assert_eq!(&value[36..38], &wanted.address.port().to_be_bytes());
        task.abort();
    }

    #[tokio::test]
    async fn none_retracts() {
        // given: an old advert of this node lies in the replica
        let fx = setup();
        fx.write_advert(advert(4)).await;
        // when
        let (_adverts, task) = fx.run(None);
        // then
        fx.expect_advert(None).await;
        task.abort();
    }

    #[tokio::test]
    async fn new_advert_overwrites() {
        // given: the relay is advertised
        let fx = setup();
        let (adverts, task) = fx.run(Some(advert(4)));
        fx.expect_advert(Some(advert(4))).await;
        // when
        adverts.send(Some(advert(5))).expect("the task listens");
        // then
        fx.expect_advert(Some(advert(5))).await;
        task.abort();
    }

    #[tokio::test]
    async fn address_change_rewrites_silently() {
        // given: the relay is advertised
        let fx = setup();
        let (adverts, task) = fx.run(Some(advert(4)));
        fx.expect_advert(Some(advert(4))).await;
        // when: the same relay reports a new address
        let moved = RelayAdvert {
            address: SocketAddrV4::new(Ipv4Addr::new(10, 9, 9, 9), 5000),
            ..advert(4)
        };
        adverts.send(Some(moved)).expect("the task listens");
        // then: the record follows, and no log line is due for it
        fx.expect_advert(Some(moved)).await;
        assert_eq!(relay_line(Some(advert(4)), Some(moved)), None);
        task.abort();
    }

    #[tokio::test]
    async fn advert_that_vanishes_retracts_the_record() {
        // given
        let fx = setup();
        let (adverts, task) = fx.run(Some(advert(4)));
        fx.expect_advert(Some(advert(4))).await;
        // when
        adverts.send(None).expect("the task listens");
        // then
        fx.expect_advert(None).await;
        task.abort();
    }

    #[tokio::test]
    async fn ends_when_the_sender_is_dropped() {
        // given
        let fx = setup();
        let (adverts, task) = fx.run(Some(advert(4)));
        fx.expect_advert(Some(advert(4))).await;
        // when
        drop(adverts);
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
        fx.write_advert(advert(4)).await;
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
        fx.write_advert(advert(4)).await;
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
    /// Starts the task with the first advert `first`; the sender is the handle to change it.
    fn run(
        &self,
        first: Option<RelayAdvert>,
    ) -> (watch::Sender<Option<RelayAdvert>>, JoinHandle<()>) {
        let (tx, rx) = watch::channel(first);
        let task = crate::node::output::spawn(advertise_relay(rx, self.replica.clone()));
        (tx, task)
    }

    async fn write_advert(&self, advert: RelayAdvert) {
        self.replica
            .set_own(RELAY_ADVERT_KEY, encode_relay_value(&advert))
            .await
            .expect("write");
    }

    /// The bytes of the own record, `None` when there is no live record.
    async fn raw_advert(&self) -> Option<Bytes> {
        let own = own_value(&*self.replica, RELAY_ADVERT_KEY)
            .await
            .expect("read");
        own.map(|(value, _)| value)
    }

    /// The own advert as a reader sees it: `None` when there is no live record or it has no address.
    async fn advert(&self) -> Option<RelayAdvert> {
        let value = self.raw_advert().await?;
        let (key, address) = decode_relay_value(&value)?;
        Some(RelayAdvert {
            key,
            address: address?,
        })
    }

    /// Waits until the own advert is `want`.
    async fn expect_advert(&self, want: Option<RelayAdvert>) {
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

/// A distinct relay per `n`: key `[n; 32]`, address `10.0.0.n:4000+n`.
fn advert(n: u8) -> RelayAdvert {
    RelayAdvert {
        key: [n; 32],
        address: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, n), 4000 + u16::from(n)),
    }
}

mod relay_line {
    use super::*;

    #[test]
    fn appearing_relay_says_serving() {
        // when
        let line = relay_line(None, Some(advert(1)));
        // then
        let want = format!("serving as relay for members of network {NET_NAME}");
        assert_eq!(line, Some(want));
    }

    #[test]
    fn vanishing_relay_says_stopped() {
        assert_eq!(
            relay_line(Some(advert(1)), None),
            Some("stopped serving as relay".to_string())
        );
    }

    #[test]
    fn another_relay_or_address_is_silent() {
        assert_eq!(relay_line(Some(advert(1)), Some(advert(2))), None);
    }

    #[test]
    fn unchanged_state_is_silent() {
        assert_eq!(relay_line(None, None), None);
        assert_eq!(relay_line(Some(advert(1)), Some(advert(1))), None);
    }
}
