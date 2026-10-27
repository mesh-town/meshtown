use super::*;

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
}

#[test]
fn terminate_reply_now_without_quit() {
    assert_eq!(
        terminate_reply(None),
        NSApplicationTerminateReply::TerminateNow
    );
}
