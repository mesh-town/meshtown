//! The client's own count of exit refusals, written by `try_exit` (see `exit/load/client.rs`).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::path_busy_tests::config_with;
use crate::exit::test_support::Scripted;
use crate::exit::{self, Answer, EXIT_CODE_OK, EXIT_PROTOCOL_VERSION};
use crate::proxy::path::*;
use crate::proxy::serve::ProxyConfig;
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::ServiceHandler;

const BUSY_REPLY: &[u8] = &[EXIT_PROTOCOL_VERSION];
const OK_REPLY: &[u8] = &[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK];
const FORBIDDEN: &[u8] = &[EXIT_PROTOCOL_VERSION, exit::EXIT_CODE_ADDRESS_FORBIDDEN];

async fn ask(cfg: &ProxyConfig, candidates: &[crate::transport::net::PeerId], times: usize) {
    for _ in 0..times {
        let res = within(
            "open_through_exits",
            open_through_exits(cfg, candidates, "example.com", 443),
        )
        .await;
        drop(res);
    }
}

/// given: the only exit never gives a stream within the pool timeout; when asked ten times, then the deadlines
/// are neither attempts nor refusals: the exit is not marked.
#[tokio::test]
async fn stream_deadline_is_not_counted() {
    let cfg = config_with(&[2], || Arc::new(Scripted::new(OK_REPLY))).await;

    ask(&cfg, &[pid(2)], 10).await;

    assert_eq!(cfg.pool.exit_busy(pid(2), Instant::now()), None);
    cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    // five refusals among five attempts: had the ten deadlines counted as attempts, the share would be 33 %,
    // had they counted as refusals, the exit would have been marked before.
    assert_eq!(cfg.pool.exit_busy(pid(2), Instant::now()), Some((5, 5)));
}

/// given: the exit answers with a refusal code of the target; when asked 45 times and then five "busy"
/// answers are added, then the 45 were attempts (11 % is below the share) and not refusals.
#[tokio::test]
async fn exit_reply_code_counts_as_attempt() {
    let cfg = config_with(&[], || Arc::new(Scripted::new(FORBIDDEN))).await;

    ask(&cfg, &[pid(2)], 45).await;
    assert_eq!(cfg.pool.exit_busy(pid(2), Instant::now()), None);
    for _ in 0..5 {
        cfg.pool.note_answer(pid(2), Answer::Busy, Instant::now());
    }

    assert_eq!(cfg.pool.exit_busy(pid(2), Instant::now()), None);
}

/// given: exit 2 answers "busy", exit 3 is alive; when five requests go through `[2, 3]`, then each opens on
/// exit 3 and the refusals are recorded against exit 2 only.
#[tokio::test]
async fn busy_exit_is_counted_then_next_exit_is_tried() {
    let handlers: Mutex<Vec<Arc<dyn ServiceHandler>>> = Mutex::new(vec![
        Arc::new(Scripted::new(OK_REPLY)),
        Arc::new(Scripted::new(BUSY_REPLY)),
    ]);
    let cfg = config_with(&[], || handlers.lock().unwrap().pop().expect("two exits")).await;

    for _ in 0..5 {
        let res = within(
            "open_through_exits",
            open_through_exits(&cfg, &[pid(2), pid(3)], "example.com", 443),
        )
        .await;
        assert!(res.is_ok(), "the second exit opens the stream");
    }

    assert_eq!(cfg.pool.exit_busy(pid(2), Instant::now()), Some((5, 5)));
    assert_eq!(cfg.pool.exit_busy(pid(3), Instant::now()), None);
}
