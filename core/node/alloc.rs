//! The node's allocator and the heap profile for the memory guard (`guard`).
//!
//! Why: when an exit node grows without a named owner, the heap profile at the moment of
//! `[guard]` names the allocation site. Only a build with the `heap-prof` feature on Linux installs jemalloc
//! (with `profiling`); profiling is switched on by an environment variable
//! `_RJEM_MALLOC_CONF=prof:true,prof_active:true,lg_prof_sample:19` (the `_rjem_` prefix is set by
//! `tikv-jemalloc-sys`), without it there is no overhead. Other builds (macOS, Windows, router) don't change the allocator.
//! Links: `guard` calls `heap_prof_state` at startup and `heap_prof_dump` from the summary thread.

#[cfg(all(feature = "heap-prof", target_os = "linux"))]
mod on {
    use std::ffi::CString;
    use std::os::raw::c_char;

    use tikv_jemalloc_ctl::raw;

    #[global_allocator]
    static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

    /// Whether the heap profile is on (`opt.prof`).
    fn enabled() -> bool {
        // SAFETY: `opt.prof` is a bool according to the jemalloc documentation.
        unsafe { raw::read::<bool>(b"opt.prof\0") }.unwrap_or(false)
    }

    pub(super) fn state() -> &'static str {
        match enabled() {
            true => "on",
            false => "off (no _RJEM_MALLOC_CONF)",
        }
    }

    pub(super) fn dump(path: &str) -> Result<(), String> {
        if !enabled() {
            return Err("profile is off (no _RJEM_MALLOC_CONF)".into());
        }
        let c = CString::new(path).map_err(|e| e.to_string())?;
        let ptr: *const c_char = c.as_ptr();
        // SAFETY: `prof.dump` takes a `const char *` — the path; the string lives until the call ends.
        unsafe { raw::write(b"prof.dump\0", ptr) }.map_err(|e| e.to_string())
    }
}

#[cfg(not(all(feature = "heap-prof", target_os = "linux")))]
mod on {
    pub(super) fn state() -> &'static str {
        "build without heap-prof"
    }

    pub(super) fn dump(_path: &str) -> Result<(), String> {
        Err("build without heap-prof".into())
    }
}

/// Profile state for the guard's start line: "on", "off (…)", "build without heap-prof".
pub(crate) fn heap_prof_state() -> &'static str {
    on::state()
}

/// Writes the heap profile dump to the file `path`.
pub(crate) fn heap_prof_dump(path: &str) -> Result<(), String> {
    on::dump(path)
}
