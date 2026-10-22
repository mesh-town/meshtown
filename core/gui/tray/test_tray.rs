//! A `TrayBackend` stub for tests: no event loop, no browser, no clipboard.

use super::backend::TrayBackend;
use super::core::Core;

/// The backend handed to `channel` in tests.
pub(crate) struct TestTray;

/// The shared test backend.
pub(crate) static TEST_TRAY: TestTray = TestTray;

impl TrayBackend for TestTray {
    fn run(&self, _core: Core) -> ! {
        unreachable!("tests never start the event loop")
    }

    fn wake(&self) {}

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Ok(())
    }

    fn copy_to_clipboard(&self, _text: &str) -> Result<(), &'static str> {
        Ok(())
    }
}
