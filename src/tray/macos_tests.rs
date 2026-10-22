use super::*;

#[test]
fn wake_without_block_is_noop() {
    // In the test binary `WAKE_BLOCK` is never set.
    wake();
    assert!(WAKE_BLOCK.get().is_none());
}
