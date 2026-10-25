//! Unblocks the stop signals a parent may have left blocked.
//! What it does: removes SIGTERM and SIGINT from the signal mask of the calling thread before the runtime exists,
//! so every thread the runtime starts inherits an open mask. A signal mask, unlike an ignored signal, survives
//! `exec`, and the runtime's signal handlers do not clear it; a blocked SIGTERM would otherwise never arrive.
//! What it doesn't do: it doesn't install handlers (the node does, in `core/node/host/signal.rs`).

/// Unblocks SIGTERM and SIGINT for the calling thread; a no-op where the platform has no signal masks.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "android"))]
pub(crate) fn unblock_stop_signals() {
    // SAFETY: `set` is initialised by `sigemptyset` before use, and the old mask is not requested.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
pub(crate) fn unblock_stop_signals() {}
