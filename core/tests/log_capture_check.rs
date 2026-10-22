//! Check of the layer `tests/common/log_capture.rs` itself: `info` is not counted, `warn` and `error`
//! are counted, `mark` cuts off the old ones. Exact counts hold under `nextest` (a process per test); under
//! `cargo test` the file's two tests share a process and their events mix.

#[path = "common/log_capture.rs"]
mod log_capture;

use tracing::Level;

#[test]
fn layer_counts_warn_and_error_after_mark_and_ignores_info() {
    // given: the layer is installed, there is already an ERROR before the mark
    assert!(
        log_capture::install(),
        "the layer must be installed in a clean process"
    );
    tracing::error!(target: "check", "old");
    let mark = log_capture::mark();

    // when: after the mark, info, warn and warn
    tracing::info!(target: "check", "just information");
    tracing::warn!(target: "check", "first warning");
    tracing::warn!(target: "check", "second warning");

    // then: info is not counted, warn counts two, no errors since the mark, the old error is not visible
    assert_eq!(log_capture::warns_since(mark), 2);
    assert_eq!(log_capture::count_since(mark, Level::INFO), 1);
    assert_eq!(log_capture::count_since(mark, Level::ERROR), 0);
    let seen = log_capture::since(mark);
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[1].message, "first warning");
    assert_eq!(seen[1].target, "check");
}

#[test]
fn assert_no_errors_since_panics_on_error_after_mark() {
    // given: a mark, then an ERROR (a separate process under nextest; under cargo test the target is its own)
    let mark = log_capture::mark();
    tracing::error!(target: "check", "new error");

    // when: the check is called
    let outcome = std::panic::catch_unwind(|| log_capture::assert_no_errors_since(mark));

    // then: it fails
    assert!(outcome.is_err(), "an ERROR after the mark must fail the check");
}
