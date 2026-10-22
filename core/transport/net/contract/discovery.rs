//! Discovery contract suite: the `discovery_contract!` macro and helpers for the "what is published is found" case.
//! Plugging in an adapter: `discovery_contract!(fn_without_arguments_returning_impl_DiscoveryHarness);`.

use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::contract::{pid, STEP};
use crate::transport::net::discovery::Discovery;
use crate::transport::net::id::PeerId;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Node address from a seed: an id and one ordinary hint.
pub(crate) fn addr_of(seed: u8) -> PeerAddr {
    PeerAddr {
        id: pid(seed),
        hints: vec![AddrHint::Ip(
            "192.0.2.7:4000".parse().expect("sample address"),
        )],
    }
}

/// A started `run_source` of a source and the receiver of what it found; `run_source` is dropped together with the stream.
pub(crate) struct Feed {
    rx: mpsc::Receiver<PeerAddr>,
    task: JoinHandle<()>,
}

/// Starts a source's `run_source` in a separate task.
pub(crate) fn feed(source: &Arc<dyn Discovery>) -> Feed {
    let (tx, rx) = mpsc::channel(64);
    let task = tokio::spawn(source.clone().run_source(tx));
    Feed { rx, task }
}

impl Feed {
    /// Returns whether an address with this id appeared within the step deadline; repeats and foreign addresses are skipped.
    pub(crate) async fn sees(&mut self, id: PeerId) -> bool {
        let found = tokio::time::timeout(STEP, self.until(id)).await;
        found.unwrap_or(None).is_some()
    }

    async fn until(&mut self, id: PeerId) -> Option<PeerAddr> {
        loop {
            let addr = self.rx.recv().await?;
            if addr.id == id {
                return Some(addr);
            }
        }
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.task.abort();
    }
}

macro_rules! discovery_contract {
    ($harness:path) => {
        mod discovery {
            use super::*;
            use $crate::transport::net::contract::discovery::{addr_of, feed};
            use $crate::transport::net::contract::{must_finish, pid, DiscoveryHarness, SEED_A};

            mod publisher {
                use super::*;

                #[tokio::test]
                async fn reports_that_it_publishes() {
                    // given: a source ordered as publishing
                    let h = $harness();
                    // when: we ask for the flag
                    let source = h.source(true);
                    // then: it publishes
                    assert!(
                        source.publishes(),
                        "publishes() of a publishing source is false"
                    );
                }

                #[tokio::test]
                async fn publish_succeeds() {
                    // given: a publishing source
                    let h = $harness();
                    let source = h.source(true);
                    // when: we publish our own address
                    let result = must_finish(source.publish(&addr_of(SEED_A))).await;
                    // then: success
                    assert!(result.is_ok(), "publish of a publishing source failed");
                }

                #[tokio::test]
                async fn address_published_after_run_started_is_found() {
                    // given: two sources on a shared environment, B already has run_source going
                    let h = $harness();
                    let (a, b) = (h.source(true), h.source(true));
                    let mut found = feed(&b);
                    // when: A publishes its own address
                    must_finish(a.publish(&addr_of(SEED_A)))
                        .await
                        .expect("publish A");
                    // then: B's run_source returns an address with A's id
                    assert!(
                        found.sees(pid(SEED_A)).await,
                        "B did not see A's address within the step deadline"
                    );
                }

                #[tokio::test]
                async fn address_published_before_run_started_is_found() {
                    // given: A has already published its own address
                    let h = $harness();
                    let (a, b) = (h.source(true), h.source(true));
                    must_finish(a.publish(&addr_of(SEED_A)))
                        .await
                        .expect("publish A");
                    // when: B's run_source only starts now
                    let mut found = feed(&b);
                    // then: B's run_source returns an address with A's id
                    assert!(
                        found.sees(pid(SEED_A)).await,
                        "B did not see A's earlier published address"
                    );
                }
            }

            mod reader {
                use super::*;

                #[tokio::test]
                async fn reports_that_it_does_not_publish() {
                    // given: a source ordered as reading
                    let h = $harness();
                    // when: we ask for the flag
                    let source = h.source(false);
                    // then: it does not publish
                    assert!(
                        !source.publishes(),
                        "publishes() of a reading source is true"
                    );
                }

                #[tokio::test]
                async fn publish_returns_ok() {
                    // given: a reading source
                    let h = $harness();
                    let source = h.source(false);
                    // when: it is told to publish an address
                    let result = must_finish(source.publish(&addr_of(SEED_A))).await;
                    // then: Ok(()), no error
                    assert!(
                        result.is_ok(),
                        "publish of a reading source returned an error"
                    );
                }

                #[tokio::test]
                async fn finds_what_a_publisher_published() {
                    // given: reading B with run_source going and publishing A on a shared environment
                    let h = $harness();
                    let (a, b) = (h.source(true), h.source(false));
                    let mut found = feed(&b);
                    // when: A publishes its own address
                    must_finish(a.publish(&addr_of(SEED_A)))
                        .await
                        .expect("publish A");
                    // then: reading B sees an address with A's id
                    assert!(
                        found.sees(pid(SEED_A)).await,
                        "reading B did not see A's address"
                    );
                }
            }
        }
    };
}

pub(crate) use discovery_contract;
