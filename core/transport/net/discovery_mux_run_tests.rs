//! Tests for discovery merging, continued: starting and stopping sources, publishing to all publishing sources.

use super::*;
use crate::transport::net::error::NetError;
use crate::transport::net::facade::test_support::{ScriptReply, STEP};
use std::time::Instant;

mod start {
    use super::*;

    #[tokio::test]
    async fn start_feeds_lookup() {
        let found = addr(2, &[1]);
        let script = Script::new("mainline")
            .finding(vec![found.clone()])
            .shared();
        let mux = started(&["mainline"], &[&script]);

        let got = within("lookup found the address", wait_lookup(&mux, 2)).await;

        assert_eq!(got, found);
    }

    #[tokio::test]
    async fn start_twice_once() {
        let script = Script::new("mainline").shared();
        let mux = started(&["mainline"], &[&script]);
        wait_until("run called", || script.runs() == 1).await;

        mux.start();
        sleep(Duration::from_millis(100)).await;

        assert_eq!(script.runs(), 1, "a second start does not start the source");
    }

    #[tokio::test]
    async fn finished_run_not_restarted() {
        let found = addr(2, &[1]);
        let script = Script::new("mainline")
            .finding(vec![found.clone()])
            .ending()
            .shared();
        let mux = started(&["mainline"], &[&script]);
        within("lookup found the address", wait_lookup(&mux, 2)).await;

        sleep(Duration::from_millis(100)).await;

        assert_eq!(script.runs(), 1, "a finished run is not restarted");
    }

    #[tokio::test]
    async fn each_source_runs_once() {
        let (one, two) = (
            Script::new("invite").shared(),
            Script::new("cache").shared(),
        );

        let _mux = started(&["invite", "cache"], &[&one, &two]);

        wait_until("both runs called", || one.runs() == 1 && two.runs() == 1).await;
    }
}

mod stop {
    use super::*;

    #[tokio::test]
    async fn stop_ends_runs_idempotent() {
        let script = Script::new("mainline")
            .finding(vec![addr(2, &[1])])
            .shared();
        let mux = started(&["mainline"], &[&script]);
        within("lookup found the address", wait_lookup(&mux, 2)).await;
        assert!(!script.dropped(), "control: run is alive before stop");

        within("first stop", mux.stop()).await;
        assert!(script.dropped(), "the source's run must be dropped");

        within("second stop", mux.stop()).await;
    }

    #[tokio::test]
    async fn o2_dropping_started_mux_drops_runs() {
        let script = Script::new("mainline")
            .finding(vec![addr(2, &[1])])
            .shared();
        let mux = started(&["mainline"], &[&script]);
        within("lookup found the address", wait_lookup(&mux, 2)).await;
        assert!(!script.dropped(), "control: run is alive before drop");

        drop(mux);

        wait_until("run dropped", || script.dropped()).await;
    }

    #[tokio::test]
    async fn stop_without_start_returns() {
        let mux = Arc::new(setup(&["invite"]));

        within("stop without start", mux.stop()).await;
    }

    #[tokio::test]
    async fn data_survives_stop() {
        let script = Script::new("mainline")
            .finding(vec![addr(2, &[1])])
            .shared();
        let mux = started(&["mainline"], &[&script]);
        within("lookup found the address", wait_lookup(&mux, 2)).await;

        within("stop", mux.stop()).await;

        assert_eq!(hints_of(&mux, 2), Some(hints(&[1])), "what was found stays");
        mux.note("manual", addr(3, &[2]));
        assert_eq!(
            hints_of(&mux, 3),
            Some(hints(&[2])),
            "note works after stop"
        );
    }
}

mod publish {
    use super::*;

    #[tokio::test]
    async fn publish_only_publishers() {
        let me = addr(1, &[1]);
        let writer = Script::new("mainline").publisher(ScriptReply::Ok).shared();
        let reader = Script::new("cache").shared();
        let mux = started(&["mainline", "cache"], &[&writer, &reader]);

        let res = within("publish", mux.publish(&me)).await;

        assert!(res.is_ok(), "publishing must succeed: {res:?}");
        assert_eq!(
            writer.published(),
            vec![me],
            "the publishing source received its own address"
        );
        assert_eq!(
            reader.publish_calls(),
            0,
            "publish is not called on a reader"
        );
    }

    #[tokio::test]
    async fn publish_none_ok() {
        let reader = Script::new("cache").shared();
        let mux = started(&["cache"], &[&reader]);

        let res = within("publish", mux.publish(&addr(1, &[1]))).await;

        assert!(res.is_ok(), "without publishing sources — Ok: {res:?}");
    }

    #[tokio::test]
    async fn publish_first_error_by_add_order() {
        let first = Script::new("a").publisher(ScriptReply::Refused);
        let first = first.replying_after(Duration::from_millis(100)).shared();
        let second = Script::new("b").publisher(ScriptReply::Timeout).shared();
        let third = Script::new("c").publisher(ScriptReply::Ok).shared();
        let mux = started(&["a", "b", "c"], &[&first, &second, &third]);

        let res = within("publish", mux.publish(&addr(1, &[1]))).await;

        assert!(
            matches!(res, Err(NetError::Refused)),
            "the error of the first by add (not by time) goes up: {res:?}"
        );
        assert_eq!(
            third.publish_calls(),
            1,
            "the refusal of earlier ones does not stop the third"
        );
    }

    #[tokio::test]
    async fn publish_timeout() {
        let stuck = Script::new("mainline")
            .publisher(ScriptReply::Forever)
            .shared();
        let timing = DiscoveryTiming {
            ttl: Duration::from_millis(TTL_MS),
            publish_timeout: Duration::from_millis(200),
        };
        let mut mux = DiscoveryMux::with_timing(vec!["mainline"], timing);
        mux.add(stuck);

        let res = within("publish", mux.publish(&addr(1, &[1]))).await;

        assert!(
            matches!(res, Err(NetError::Timeout)),
            "expected Timeout: {res:?}"
        );
    }

    #[tokio::test]
    async fn publisher_panic_is_other() {
        let broken = Script::new("mainline")
            .publisher(ScriptReply::Panic)
            .shared();
        let mux = started(&["mainline"], &[&broken]);

        let res = within("publish", mux.publish(&addr(1, &[1]))).await;

        assert!(
            matches!(res, Err(NetError::Other(_))),
            "expected Other: {res:?}"
        );
    }

    #[tokio::test]
    async fn publishers_run_together() {
        let slow = |name| {
            let script = Script::new(name).publisher(ScriptReply::Ok);
            script.replying_after(Duration::from_millis(400)).shared()
        };
        let (a, b, c) = (slow("a"), slow("b"), slow("c"));
        let mux = started(&["a", "b", "c"], &[&a, &b, &c]);
        let began = Instant::now();

        let res = within("publish", mux.publish(&addr(1, &[1]))).await;

        assert!(res.is_ok(), "publishing must succeed: {res:?}");
        let spent = began.elapsed();
        assert!(
            spent < Duration::from_millis(1000),
            "one after another would take 1200 ms, it took {spent:?}"
        );
        assert!(spent < STEP);
    }
}
