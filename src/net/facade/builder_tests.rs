//! Tests of the facade builder (`builder.rs`): settings errors, what is called during the build.

use super::NetStackBuilder;
use crate::net::error::{NetError, NetResult};
use crate::net::facade::test_support::{secret, Probe, Script, Spy};
use crate::net::facade::NetStack;
use crate::net::mem::MemNet;
use crate::net::service::{AUTH, EXIT};

mod config_errors {
    use super::*;

    #[tokio::test]
    async fn no_transport_config() {
        let builder = NetStackBuilder::new(secret(1));

        let res = builder.build();

        expect_config(res);
    }

    #[tokio::test]
    async fn foreign_key_config() {
        let net = MemNet::new();
        let foreign = Spy::on(&net, &secret(2)).shared();

        let res = NetStackBuilder::new(secret(1)).transport(foreign).build();

        expect_config(res);
    }

    #[tokio::test]
    async fn duplicate_service_config() {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();

        let res = NetStackBuilder::new(secret(1))
            .transport(mine)
            .serve(AUTH, Probe::new())
            .serve(AUTH, Probe::new())
            .build();

        expect_config(res);
    }

    #[tokio::test]
    async fn duplicate_source_config() {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();

        let res = NetStackBuilder::new(secret(1))
            .transport(mine)
            .discovery(Script::new("mainline").shared())
            .discovery(Script::new("mainline").shared())
            .build();

        expect_config(res);
    }

    #[test]
    fn outside_runtime_config() {
        // given: the transport was created in a runtime, and the build runs outside it (no tokio context)
        let rt = tokio::runtime::Runtime::new().expect("runtime did not start");
        let net = MemNet::new();
        let mine = rt.block_on(async { Spy::on(&net, &secret(1)).shared() });

        // when: build is called on a plain thread
        let res = NetStackBuilder::new(secret(1)).transport(mine).build();

        // then: a settings error, not a "no runtime" panic
        expect_config(res);
    }
}

mod accepted_setups {
    use super::*;

    #[tokio::test]
    async fn two_same_kind_ok() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let (first, second) = (Spy::on(&x, &secret(1)), Spy::on(&y, &secret(1)));

        let res = NetStackBuilder::new(secret(1))
            .transport(first.shared())
            .transport(second.shared())
            .build();

        let stack = res.unwrap_or_else(|e| panic!("two transports of one kind are allowed: {e}"));
        assert_eq!(stack.snapshot().transport_names, vec!["mem", "mem"]);
    }

    #[tokio::test]
    async fn unknown_priority_names_ignored() {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();

        let res = NetStackBuilder::new(secret(1))
            .transport(mine)
            .discovery(Script::new("mainline").shared())
            .discovery_priority(vec!["nowhere", "mainline"])
            .build();

        let stack = res.unwrap_or_else(|e| panic!("an extra name in priority is not an error: {e}"));
        assert_eq!(stack.snapshot().discovery_names, vec!["mainline"]);
    }

    #[tokio::test]
    async fn no_discovery_and_no_services_ok() {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();

        let res = NetStackBuilder::new(secret(1)).transport(mine).build();

        assert!(res.is_ok(), "one transport is enough");
    }
}

mod serving {
    use super::*;

    #[tokio::test]
    async fn every_transport_gets_all_service_ids() {
        let (x, y) = (MemNet::new(), MemNet::new());
        let (first, second) = (
            Spy::on(&x, &secret(1)).shared(),
            Spy::on(&y, &secret(1)).shared(),
        );

        let _stack = NetStackBuilder::new(secret(1))
            .transport(first.clone())
            .transport(second.clone())
            .serve(AUTH, Probe::new())
            .serve(EXIT, Probe::new())
            .build()
            .unwrap_or_else(|e| panic!("build failed: {e}"));

        for spy in [first, second] {
            let calls = spy.served();
            assert_eq!(calls.len(), 1, "serve — exactly once");
            assert_eq!(calls[0].len(), 2);
            assert!(calls[0].contains(&AUTH) && calls[0].contains(&EXIT));
        }
    }

    #[tokio::test]
    async fn serve_is_called_even_for_empty_list() {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();

        let _stack = NetStackBuilder::new(secret(1))
            .transport(mine.clone())
            .build()
            .unwrap_or_else(|e| panic!("build failed: {e}"));

        assert_eq!(
            mine.served(),
            vec![Vec::new()],
            "serve(&[]) is still called"
        );
    }
}

mod found_tap {
    use super::*;
    use crate::net::facade::test_support::{addr, within};
    use tokio::sync::mpsc;

    fn stack_with(
        script: std::sync::Arc<Script>,
        tx: mpsc::Sender<crate::net::addr::PeerAddr>,
    ) -> NetStack {
        let net = MemNet::new();
        let mine = Spy::on(&net, &secret(1)).shared();
        NetStackBuilder::new(secret(1))
            .transport(mine)
            .discovery(script)
            .found_tap(tx)
            .build()
            .unwrap_or_else(|e| panic!("build failed: {e}"))
    }

    #[tokio::test]
    async fn repeated_dht_report_reaches_dialer_twice() {
        let a = addr(2, &[1]);
        let script = Script::new("mainline")
            .finding(vec![a.clone(), a.clone()])
            .shared();
        let (tx, mut rx) = mpsc::channel(8);

        let _stack = stack_with(script, tx);

        assert_eq!(within("first", rx.recv()).await, Some(a.clone()));
        assert_eq!(within("second", rx.recv()).await, Some(a));
    }

    #[tokio::test]
    async fn found_tap_closed_stops_source() {
        let many = (1..=70).map(|n| addr(n, &[1])).collect();
        let script = Script::new("mainline").finding(many).shared();
        let (tx, rx) = mpsc::channel(8);
        drop(rx);

        let _stack = stack_with(script.clone(), tx);

        within("source stopped", async {
            while !script.dropped() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
    }
}

/// The `Config` message from an error; any other error or a success fails the test.
fn expect_config(res: NetResult<NetStack>) -> String {
    match res {
        Err(NetError::Config(msg)) => msg,
        Err(other) => panic!("expected Config, got {other:?}"),
        Ok(_) => panic!("expected Config, got a built facade"),
    }
}
