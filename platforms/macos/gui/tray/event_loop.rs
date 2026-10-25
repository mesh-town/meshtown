//! The macOS tray loop: `NSApplication` without a Dock icon, the icon in `applicationDidFinishLaunching:`,
//! `Core` commands are executed by the main loop on wakeup from any thread.
//!
//! What it does: `run` puts `Core` into the main thread's thread-local storage and starts
//! `NSApplication::run()`; the delegate sets the `Accessory` policy, calls `attach_ui`, installs the wakeup
//! block and drains the queue; `wake` from any thread puts this block onto the main
//! `CFRunLoop` and wakes it.
//! What it does not do: it does not build the menu or print itself (that is `Core` and `log_tray`), does not end the
//! process by anything other than `exit_now` on `TrayCmd::QuitWithCode`, and does not know about the node or the page.
//! Links: `Core`, `exit_now`; the model is
//! a separate repaint probe.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSStatusItem,
};
use objc2_core_foundation::{kCFRunLoopCommonModes, CFRunLoop, CFType};
use objc2_foundation::NSNotification;

use meshtown_core::gui::tray::{exit_now, log_tray, Core};

thread_local! {
    /// The main thread's `Core`: after `attach_ui` it holds a `TrayIcon` (`!Send`), so the storage is
    /// thread-local. Empty before `run` and after the fallback path takes it.
    static CORE: RefCell<Option<Core>> = const { RefCell::new(None) };
    /// The outer pass schedules one more after itself.
    static WAKE_MISSED_WHILE_BUSY: Cell<bool> = const { Cell::new(false) };
}

/// The wakeup block that crosses threads as an inert pointer.
///
/// SAFETY: the block closure captures nothing — its body only calls `drain_on_main()`,
/// which touches only the main thread's thread-local `CORE`/`WAKE_MISSED_WHILE_BUSY`; only the main
/// loop executes the block. `perform_block` is called concurrently from several threads (the poller on tokio
/// worker threads, the main one from the menu handler), and CF only copies the block (`Block_copy` —
/// an atomic refcount of the block's heap); no Rust copies of `RcBlock` are made, `Drop` of the static does not
/// happen (the process ends with `exit_now`, static destructors do not run).
struct SendSyncBlock(RcBlock<dyn Fn()>);
unsafe impl Send for SendSyncBlock {}
unsafe impl Sync for SendSyncBlock {}

/// Set once in the delegate after `attach_ui`; before that `wake()` does nothing.
static WAKE_BLOCK: OnceLock<SendSyncBlock> = OnceLock::new();

#[derive(Default)]
struct DelegateIvars {}

define_class!(
    // SAFETY:
    // - The superclass NSObject requires nothing from subclasses.
    // - `Delegate` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    struct Delegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for Delegate {}

    // SAFETY: `NSApplicationDelegate` has no safety requirements.
    unsafe impl NSApplicationDelegate for Delegate {
        // SAFETY: the signature is correct.
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let app = NSApplication::sharedApplication(self.mtm());
            // The result is not checked: if `false`, the Dock icon is seen by acceptance from the snapshot.
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

            let (attached, item) = CORE.with(|cell| match cell.borrow_mut().as_mut() {
                Some(core) => (core.attach_ui(), status_item(core)),
                None => (Err("no tray state".to_string()), None),
            });
            if let Err(e) = attached {
                log_tray(&format!("no tray icon: {e}"));
            }
            self.schedule_check(item);

            // The order "block → initial pass" is mandatory: a command between the end of the pass and
            // the block being installed would otherwise wait for the next command.
            let block: RcBlock<dyn Fn()> = RcBlock::new(drain_on_main);
            // AppKit does not make a second call; if it did happen — silently keep the first block.
            let _ = WAKE_BLOCK.set(SendSyncBlock(block));
            drain_on_main();
        }
    }

    impl Delegate {
        // SAFETY: the signature is correct.
        #[unsafe(method(checkStatusItem:))]
        fn check_status_item(&self, item: &NSStatusItem) {
            if super::hidden_icon::check(item, self.mtm()) {
                self.schedule(sel!(recheckStatusItem:), item, super::hidden_icon::RECHECK_DELAY_SECS);
            }
        }

        // SAFETY: the signature is correct.
        #[unsafe(method(recheckStatusItem:))]
        fn recheck_status_item(&self, item: &NSStatusItem) {
            super::hidden_icon::recheck(item, self.mtm());
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars::default());
        // SAFETY: the `init` signature of `NSObject` is correct.
        unsafe { msg_send![super(this), init] }
    }

    /// Schedules the first "hidden" check via `hidden_icon::CHECK_DELAY_SECS`; without
    /// an icon (`attach_ui` failed or has not built one yet) — it does not schedule.
    fn schedule_check(&self, item: Option<Retained<NSStatusItem>>) {
        let Some(item) = item else { return };
        self.schedule(
            sel!(checkStatusItem:),
            &item,
            super::hidden_icon::CHECK_DELAY_SECS,
        );
    }

    fn schedule(&self, selector: Sel, item: &NSStatusItem, delay: f64) {
        // SAFETY: both selectors are declared in `define_class!` above with an object argument; `performSelector:…
        // afterDelay:` retains `self` and `item` until the call; called on the main thread (the delegate).
        let _: () = unsafe {
            msg_send![self, performSelector: selector, withObject: item, afterDelay: delay]
        };
    }
}

/// The tray loop on the main thread; exits only through `exit_now` on `TrayCmd::QuitWithCode`.
pub(crate) fn run(core: Core) -> ! {
    let Some(mtm) = MainThreadMarker::new() else {
        fallback(core, "not the main thread");
    };
    CORE.with(|cell| *cell.borrow_mut() = Some(core));

    let app = NSApplication::sharedApplication(mtm);
    // `NSApplication` holds the delegate weakly — the binding lives to the end of `run`.
    let delegate = Delegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();

    // Someone sent `stop:` — carry on without an icon, like Linux.
    match CORE.with(|cell| cell.borrow_mut().take()) {
        Some(core) => fallback(core, "the macOS loop stopped"),
        None => exit_now(1),
    }
}

/// The native status item of the raised icon, for the "hidden" check; `None` until `attach_ui`.
fn status_item(core: &Core) -> Option<Retained<NSStatusItem>> {
    core.tray_icon()?.ns_status_item()
}

/// The fallback path without an icon: a line, then wait for `QuitWithCode` in a blocking way. The node keeps running.
fn fallback(mut core: Core, reason: &str) -> ! {
    log_tray(&format!("no tray icon: {reason}"));
    exit_now(core.recv_blocking())
}

/// Wakes the main loop and schedules a pass over the queue on it. Any thread; before the block is installed or
/// without a main `CFRunLoop` — nothing.
pub(crate) fn wake() {
    let Some(block) = WAKE_BLOCK.get() else {
        return;
    };
    let Some(run_loop) = CFRunLoop::main() else {
        return;
    };
    // SAFETY: `kCFRunLoopCommonModes` is an immutable CF constant, read as is.
    let mode: Option<&CFType> = unsafe { kCFRunLoopCommonModes }.map(|m| -> &CFType { m });
    // SAFETY: `mode` is a valid CF loop mode; the block lives to the end of the process in `WAKE_BLOCK` and
    // is executed only by the main loop (see `SendSyncBlock`). Common modes — so that the queue keeps going
    // even while a menu is open.
    unsafe { run_loop.perform_block(mode, Some(&block.0)) };
    // Without `wake_up`, `perform_block` does not wake the loop.
    run_loop.wake_up();
}

/// A pass over the command queue; the main thread only (the block and the delegate). `QuitWithCode` → `exit_now` right
/// here. A nested call (AppKit spun the loop inside `drain_commands`) does not panic on the busy `CORE` but
/// sets a flag; the outer pass schedules one more after itself through `wake()`.
fn drain_on_main() {
    let busy = CORE.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => {
            if let Some(core) = slot.as_mut() {
                if let Some(code) = core.drain_commands() {
                    exit_now(code);
                }
            }
            false
        }
        Err(_) => true,
    });
    if busy {
        WAKE_MISSED_WHILE_BUSY.with(|p| p.set(true));
        return;
    }
    if WAKE_MISSED_WHILE_BUSY.with(|p| p.replace(false)) {
        wake();
    }
}

#[cfg(test)]
#[path = "event_loop_tests.rs"]
mod tests;
