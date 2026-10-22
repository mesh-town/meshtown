//! Tray loop for Windows: the thread message queue, the `PostThreadMessageW` wake-up and
//! the 1 s safety timer.
//!
//! What it does: brings up COM and the thread message queue, publishes the thread id for `wake`, sets
//! a windowless thread timer, brings up the icon (an error is not fatal) and runs `GetMessageW`.
//! Thread messages (`WM_WAKE`, `WM_TIMER` without a window) are handled by `Core::drain_commands` right here, in the body of
//! the loop; the rest go to the tray-icon/muda window procedure. On `QuitWithCode` it removes the icon
//! (`drop_ui`, otherwise a "ghost" stays in the tray) and exits through `exit_now`.
//! What it does not do: it prints nothing except the no-icon line through `super::log_tray`; it knows no window
//! procedure or crate window of its own; `TaskbarCreated` (an Explorer restart) is up to tray-icon;
//! it does not send `WM_QUIT` itself.
//! Relations: `super::Core`, `super::log_tray`, `crate::app::quit::exit_now`; `wake` calls `CmdTx::send`
//! from any thread.

use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::System::Com::{
    CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, SetTimer, TranslateMessage,
    MSG, PM_NOREMOVE, WM_APP, WM_TIMER,
};

use super::Core;

/// 0 — the loop is not published yet (the queue may not exist).
static LOOP_THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// Thread message "there are commands in the channel".
const WM_WAKE: u32 = WM_APP + 1;
/// Period of the safety timer: picks up commands whose wake-up was lost (a modal menu,
/// a full queue).
const SAFETY_TICK_MS: u32 = 1000;

pub(super) fn run(mut core: Core) -> ! {
    let mut msg = MSG::default();

    // SAFETY: the Win32 calls take no pointers to Rust data except `&mut msg` — a live `MSG` on
    // this thread's stack. The COM and `PeekMessageW` results are not needed: COM is needed only by
    // `ShellExecuteW` (`open_url` reports its error), and `PeekMessageW` only creates the queue.
    unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    // Only after `PeekMessageW`: without a queue `PostThreadMessageW` from `wake` would fail.
    // SAFETY: no arguments, always succeeds.
    LOOP_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    // A windowless, procedure-less thread timer: `WM_TIMER` arrives in the queue with an empty `hwnd`.
    // If it returned 0, we work without the safety net and only `wake` wakes the loop.
    // SAFETY: an empty window and an empty procedure are allowed by the `SetTimer` contract.
    unsafe {
        SetTimer(std::ptr::null_mut(), 0, SAFETY_TICK_MS, None);
    }

    // The icon is created on this thread: the crate's hidden window belongs to it.
    if let Err(e) = core.attach_ui() {
        super::log_tray(&format!("no tray icon: {e}"));
    }
    // Commands sent before the thread id was published: `wake` did nothing for them.
    drain_or_exit(&mut core);

    loop {
        // SAFETY: `&mut msg` is a live `MSG` on the stack; an empty window means all thread messages.
        let got = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if got == 0 || got == -1 {
            // We never send `WM_QUIT` — it is foreign or an emergency; `GetMessageW` must not be called after `-1`
            // (a busy loop). The node keeps running, the icon stops responding.
            break;
        }
        if msg.hwnd.is_null() && (msg.message == WM_WAKE || msg.message == WM_TIMER) {
            // Thread message: `DispatchMessageW` would not deliver it anywhere.
            drain_or_exit(&mut core);
        } else {
            // SAFETY: `msg` was just filled by `GetMessageW`.
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    let code = core.recv_blocking();
    finish(&mut core, code)
}

/// The queue is drained only from the loop body, not from the window procedure: `&mut Core` is not taken
/// again from inside `DispatchMessageW`.
fn drain_or_exit(core: &mut Core) {
    if let Some(code) = core.drain_commands() {
        finish(core, code);
    }
}

/// Removes the icon (on Windows `Drop` is fast: `NIM_DELETE` + `DestroyWindow`), then exits.
fn finish(core: &mut Core, code: i32) -> ! {
    core.drop_ui();
    crate::app::quit::exit_now(code)
}

/// Wakes the loop with a thread message. Before the id is published it does nothing: the initial `drain_commands` picks the commands up.
/// A `PostThreadMessageW` failure (the queue is full) does not matter: the command is already in the channel, and the
/// timer tick will drain it.
pub(super) fn wake() {
    let id = LOOP_THREAD_ID.load(Ordering::SeqCst);
    if id != 0 {
        // SAFETY: no pointers; a wrong id gives an error, not UB.
        unsafe {
            PostThreadMessageW(id, WM_WAKE, 0, 0);
        }
    }
}
