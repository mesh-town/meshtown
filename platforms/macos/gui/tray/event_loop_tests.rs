use super::*;
use std::future::Future;

#[test]
fn wake_without_block_is_noop() {
    // In the test binary `WAKE_BLOCK` is never set.
    wake();
    assert!(WAKE_BLOCK.get().is_none());
}

// `request()` starts a watchdog thread that ends the process after a while; these tests are short.
#[test]
fn terminate_reply_later_when_quit_known() {
    let quit = QuitSignal::new();
    let reply = terminate_reply(Some(&quit));
    assert_eq!(reply, NSApplicationTerminateReply::TerminateLater);
    // `request()` was called: a single poll of `wait()` is already ready.
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut wait = std::pin::pin!(quit.wait());
    let seen = wait.as_mut().poll(&mut cx);
    assert!(seen.is_ready(), "the quit was not requested");
}

#[test]
fn terminate_reply_now_without_quit() {
    assert_eq!(
        terminate_reply(None),
        NSApplicationTerminateReply::TerminateNow
    );
}
