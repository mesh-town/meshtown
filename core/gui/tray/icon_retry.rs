//! Retrying the tray icon raise: on Linux the StatusNotifierWatcher may appear after the app starts
//! (an autostarted session), so a failed raise is repeated with a growing pause.
//!
//! What it does: `raise_with_retry` calls the host's `try_attach`, waits between failures through
//! `IconHost::wait` (which also handles commands and may ask to quit) and logs a few lines in total.
//! What it does not do: it does not classify the error and does not touch the menu; `Core` is one host.
//! Links: `Core` implements `IconHost` in `core_wait.rs`; the Linux loop calls `raise_with_retry`.

use std::time::Duration;

/// What the retry loop needs from the place that owns the icon.
pub trait IconHost {
    /// One attempt to raise the icon.
    fn try_attach(&mut self) -> Result<(), String>;
    /// Pauses for `pause`, handling commands meanwhile; `Some(code)` asks to quit with that code.
    fn wait(&mut self, pause: Duration) -> Option<i32>;
    /// Writes one log line.
    fn log(&mut self, text: &str);
}

/// The retry schedule: the first pause, the pause ceiling and the attempt limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPlan {
    pub first: Duration,
    pub cap: Duration,
    pub max_attempts: u32,
}

impl RetryPlan {
    /// 1 s doubling up to 30 s, at most 25 attempts (about ten minutes of waiting).
    pub const LINUX: RetryPlan = RetryPlan {
        first: Duration::from_secs(1),
        cap: Duration::from_secs(30),
        max_attempts: 25,
    };

    /// The pause after failed attempt number `attempt` (from 1): `first * 2^(attempt-1)`, at most `cap`.
    pub fn delay(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        self.first.saturating_mul(factor).min(self.cap)
    }
}

/// How the retry loop ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Raise {
    Attached { attempts: u32 },
    Quit(i32),
    GaveUp { attempts: u32 },
}

/// Tries to raise the icon, waiting and retrying after each failure until the plan runs out.
pub fn raise_with_retry<H: IconHost>(host: &mut H, plan: RetryPlan) -> Raise {
    let max = plan.max_attempts.max(1);
    let mut attempt = 1;
    loop {
        let Err(e) = host.try_attach() else {
            return attached(host, attempt);
        };
        if attempt >= max {
            host.log(&format!(
                "no tray icon: {e}; gave up after {attempt} attempts"
            ));
            return Raise::GaveUp { attempts: attempt };
        }
        if attempt == 1 {
            host.log(&format!(
                "no tray icon yet: {e}; retrying for up to {max} attempts"
            ));
        }
        if let Some(code) = host.wait(plan.delay(attempt)) {
            return Raise::Quit(code);
        }
        attempt += 1;
    }
}

fn attached<H: IconHost>(host: &mut H, attempts: u32) -> Raise {
    if attempts >= 2 {
        host.log(&format!("tray icon raised on attempt {attempts}"));
    }
    Raise::Attached { attempts }
}

#[cfg(test)]
#[path = "icon_retry_tests.rs"]
mod tests;
