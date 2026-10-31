//! Tests of the lane pool on an in-memory network: how many streams the exit sees and when they go away.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::super::failure::Failure;
use super::super::test_support::{exit_pair, exit_pair_with_stack, HoldOpen, OK};
use super::*;
use crate::exit::test_support::Scripted;
use crate::exit::{
    ExitBusy, PeerConnPool, StreamBusy, EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_PROTOCOL_VERSION,
};
use crate::transport::net::facade::test_support::within;
use crate::transport::net::NetStack;
use crate::transport::net::ServiceHandler;
use tokio::io::AsyncReadExt;
use tokio::time::sleep;

const REFUSED: &[u8] = &[EXIT_PROTOCOL_VERSION, EXIT_CODE_ADDRESS_FORBIDDEN];
const BUSY: &[u8] = &[EXIT_PROTOCOL_VERSION];
const SLOW: Duration = Duration::from_millis(200);

type Answer = Result<(BoxSend, BoxRecv, u8)>;

struct Rig {
    stack: NetStack,
    pool: PeerConnPool,
    peer: PeerId,
}

async fn rig(handler: Arc<dyn ServiceHandler>) -> Rig {
    let (stack, pool, peer) = exit_pair(handler).await;
    Rig { stack, pool, peer }
}

impl Rig {
    async fn attach(&self) -> Answer {
        within("attach", attach(&self.stack, &self.pool, self.peer)).await
    }

    async fn attach_ok(&self) -> (BoxSend, BoxRecv) {
        let (send, recv, code) = self.attach().await.expect("attached");
        assert_eq!(code, EXIT_CODE_OK);
        (send, recv)
    }

    /// `n` attaches at once.
    async fn burst(&self, n: usize) -> Vec<Answer> {
        let tasks: Vec<_> = (0..n)
            .map(|_| {
                let (stack, pool, peer) = (self.stack.clone(), self.pool.clone(), self.peer);
                crate::node::output::spawn(async move { attach(&stack, &pool, peer).await })
            })
            .collect();
        let mut answers = Vec::new();
        for task in tasks {
            answers.push(within("burst attach", async { task.await.expect("task") }).await);
        }
        answers
    }

    fn lanes(&self) -> usize {
        self.pool.udpgw.lanes(self.peer)
    }
}

fn hits(exit: &HoldOpen) -> usize {
    exit.hits.load(Ordering::SeqCst)
}

async fn sees_the_end(recv: &mut BoxRecv) {
    let end = tokio::time::timeout(Duration::from_secs(1), recv.read_u8()).await;
    assert!(end.expect("ends in time").is_err());
}

fn is_busy(answer: &Answer) -> bool {
    answer
        .as_ref()
        .err()
        .is_some_and(|e| e.downcast_ref::<ExitBusy>().is_some())
}

mod opening {
    use super::*;

    #[tokio::test]
    async fn first_attach_opens_one_stream() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;

        let _first = rig.attach_ok().await;

        assert_eq!((hits(&exit), rig.lanes()), (1, 1));
    }

    #[tokio::test]
    async fn four_locals_share_one_stream() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;

        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(rig.attach_ok().await);
        }
        sleep(Duration::from_millis(100)).await;

        assert_eq!((hits(&exit), rig.lanes()), (1, 1));
    }

    #[tokio::test]
    async fn fifth_local_opens_a_second_stream_in_the_background() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(rig.attach_ok().await);
        }

        held.push(rig.attach_ok().await);

        within("second stream", async {
            while rig.lanes() < 2 {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert_eq!(hits(&exit), 2);
    }

    #[tokio::test]
    async fn streams_never_exceed_the_lane_limit() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;

        let answers = rig.burst(20).await;
        sleep(Duration::from_millis(300)).await;

        assert!(answers.iter().all(|a| a.is_ok()));
        assert!(hits(&exit) <= LANES_PER_EXIT, "hits {}", hits(&exit));
    }

    #[tokio::test]
    async fn burst_of_first_attaches_opens_one_stream() {
        let exit = Arc::new(HoldOpen::new(OK, SLOW));
        let rig = rig(exit.clone()).await;

        let answers = rig.burst(4).await;

        assert!(answers.iter().all(|a| a.is_ok()));
        assert_eq!(hits(&exit), 1);
    }

    #[tokio::test]
    async fn busy_extra_stream_keeps_attaching_to_the_open_one() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO).then(BUSY));
        let rig = rig(exit.clone()).await;

        let mut held = Vec::new();
        for _ in 0..8 {
            held.push(rig.attach_ok().await);
            sleep(Duration::from_millis(30)).await;
        }

        assert_eq!((rig.lanes(), hits(&exit)), (1, 2));
    }

    #[tokio::test]
    async fn refused_extra_stream_pauses_and_keeps_the_open_one() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO).then(REFUSED));
        let rig = rig(exit.clone()).await;

        let mut held = Vec::new();
        for _ in 0..8 {
            held.push(rig.attach_ok().await);
            sleep(Duration::from_millis(30)).await;
        }

        assert_eq!((rig.lanes(), hits(&exit)), (1, 2));
    }

    #[tokio::test]
    async fn many_locals_open_exactly_three_streams() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;

        let mut held = Vec::new();
        for _ in 0..20 {
            held.push(rig.attach_ok().await);
            sleep(Duration::from_millis(30)).await;
        }
        sleep(Duration::from_millis(100)).await;

        assert_eq!((rig.lanes(), hits(&exit)), (LANES_PER_EXIT, LANES_PER_EXIT));
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn busy_first_stream_is_a_busy_error() {
        let rig = rig(Arc::new(Scripted::new(BUSY))).await;

        let answer = rig.attach().await;

        assert!(is_busy(&answer));
        assert_eq!(rig.lanes(), 0);
    }

    #[tokio::test]
    async fn waiter_behind_a_failed_open_does_not_ask() {
        let exit = Arc::new(HoldOpen::new(BUSY, SLOW));
        let rig = rig(exit.clone()).await;

        let answers = rig.burst(4).await;

        assert!(answers.iter().all(is_busy));
        assert_eq!(hits(&exit), 1);
    }

    #[tokio::test]
    async fn waiter_behind_a_gateway_refusal_gets_the_code() {
        let exit = Arc::new(HoldOpen::new(REFUSED, SLOW));
        let rig = rig(exit.clone()).await;

        let answers = rig.burst(4).await;

        let codes: Vec<_> = answers
            .iter()
            .map(|a| a.as_ref().ok().map(|r| r.2))
            .collect();
        assert_eq!(codes, vec![Some(EXIT_CODE_ADDRESS_FORBIDDEN); 4]);
        assert_eq!((hits(&exit), rig.lanes()), (1, 0));
    }

    #[tokio::test]
    async fn gateway_refusal_returns_the_code_without_a_stream() {
        let rig = rig(Arc::new(Scripted::new(REFUSED))).await;

        let (_, _, code) = rig.attach().await.expect("an answer");

        assert_eq!((code, rig.lanes()), (EXIT_CODE_ADDRESS_FORBIDDEN, 0));
    }

    #[tokio::test]
    async fn connection_after_a_failed_open_asks_again() {
        let exit = Arc::new(HoldOpen::new(BUSY, Duration::ZERO).then(OK));
        let rig = rig(exit.clone()).await;

        assert!(is_busy(&rig.attach().await));
        let _held = rig.attach_ok().await;

        assert_eq!((hits(&exit), rig.lanes()), (2, 1));
    }

    #[test]
    fn failure_hands_waiters_the_same_error() {
        let stream_busy = StreamBusy {
            peer: "p".into(),
            secs: 10,
            open_bi_waiter_count: 2,
        };
        let err = Failure::of(&anyhow::Error::from(stream_busy))
            .answer()
            .err()
            .expect("an error");
        let got = err.downcast_ref::<StreamBusy>().expect("a stream busy");
        assert_eq!((got.secs, got.open_bi_waiter_count), (10, 2));

        let err = Failure::of(&anyhow::anyhow!("boom"))
            .answer()
            .err()
            .expect("an error");
        assert!(format!("{err:#}").contains("boom"));
        assert!(err.downcast_ref::<ExitBusy>().is_none());
        assert!(err.downcast_ref::<StreamBusy>().is_none());
    }

    #[tokio::test]
    async fn stream_that_ends_right_after_ok_still_gives_a_pair() {
        let rig = rig(Arc::new(Scripted::new(OK))).await;

        let (_send, mut recv) = rig.attach_ok().await;

        sees_the_end(&mut recv).await;
    }
}

mod leaving {
    use super::*;

    #[tokio::test]
    async fn idle_stream_closes_after_the_last_local_leaves() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let mut rig = rig(exit.clone()).await;
        rig.pool.udpgw = rig
            .pool
            .udpgw
            .clone()
            .with_lane_idle(Duration::from_millis(100));
        drop(rig.attach_ok().await);

        within("lane removal", async {
            while rig.lanes() > 0 {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await;

        assert_eq!(rig.lanes(), 0);
    }

    #[tokio::test]
    async fn stream_with_a_local_outlives_the_idle_time() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let mut rig = rig(exit.clone()).await;
        rig.pool.udpgw = rig
            .pool
            .udpgw
            .clone()
            .with_lane_idle(Duration::from_millis(100));
        let _local = rig.attach_ok().await;

        sleep(Duration::from_millis(500)).await;

        assert_eq!(rig.lanes(), 1);
    }

    #[tokio::test]
    async fn retire_keeps_a_lane_that_got_a_local() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;
        let _local = rig.attach_ok().await;
        let lane = lock(&rig.pool.udpgw.peers)[&rig.peer].lanes[0].clone();

        assert!(!rig.pool.udpgw.retire(rig.peer, &lane));

        assert_eq!(rig.lanes(), 1);
    }

    #[tokio::test]
    async fn stream_closed_by_the_exit_ends_locals_and_reopens() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let rig = rig(exit.clone()).await;
        let (_send, mut recv) = rig.attach_ok().await;

        exit.drop_all();

        sees_the_end(&mut recv).await;
        let _again = rig.attach_ok().await;
        assert_eq!(hits(&exit), 2);
    }

    #[tokio::test]
    async fn exit_node_stop_ends_locals() {
        let exit = Arc::new(HoldOpen::new(OK, Duration::ZERO));
        let (stack, pool, peer, server) = exit_pair_with_stack(exit.clone()).await;
        let (_send, mut recv, _) = attach(&stack, &pool, peer).await.expect("attached");

        server.close().await;

        sees_the_end(&mut recv).await;
    }
}
