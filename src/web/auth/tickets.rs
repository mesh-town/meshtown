//! Temporary tokens and sessions of the page: in memory only, a restart = everyone signed out.
//! Time is the `now` parameter, there are no clocks inside.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::web::guard::ct_eq;

pub(crate) const TEMP_TTL: Duration = Duration::from_secs(600);
pub(crate) const TEMP_MAX: usize = 8;
pub(crate) const SESSION_TTL: Duration = Duration::from_secs(604_800);
pub(crate) const SESSION_MAX: usize = 64;

/// How the credentials were obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Via {
    PermanentToken,
    TempToken,
    Password,
}

impl Via {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Via::PermanentToken => "token",
            Via::TempToken => "temp",
            Via::Password => "password",
        }
    }
}

pub(crate) struct Tickets {
    temps: Vec<(String, Instant)>,
    sessions: HashMap<String, (Via, Instant)>,
}

fn alive(born: Instant, ttl: Duration, now: Instant) -> bool {
    now.saturating_duration_since(born) < ttl
}

impl Tickets {
    pub(crate) fn new() -> Self {
        Tickets {
            temps: Vec::new(),
            sessions: HashMap::new(),
        }
    }

    fn sweep(&mut self, now: Instant) {
        self.temps.retain(|(_, born)| alive(*born, TEMP_TTL, now));
        self.sessions
            .retain(|_, (_, born)| alive(*born, SESSION_TTL, now));
    }

    pub(crate) fn issue_temp(&mut self, now: Instant) -> Result<String, String> {
        let token = crate::web::token::generate()?;
        self.sweep(now);
        if self.temps.len() >= TEMP_MAX {
            self.evict_oldest_temp();
        }
        self.temps.push((token.clone(), now));
        Ok(token)
    }

    fn evict_oldest_temp(&mut self) {
        let oldest = self
            .temps
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, born))| *born)
            .map(|(i, _)| i);
        if let Some(i) = oldest {
            self.temps.remove(i);
        }
    }

    /// Compares against all live ones without an early exit; a matching one is deleted.
    pub(crate) fn take_temp(&mut self, t: &str, now: Instant) -> bool {
        self.sweep(now);
        let mut found = None;
        for (i, (cand, _)) in self.temps.iter().enumerate() {
            if ct_eq(cand.as_bytes(), t.as_bytes()) {
                found = Some(i);
            }
        }
        found.map(|i| self.temps.remove(i)).is_some()
    }

    pub(crate) fn open_session(&mut self, via: Via, now: Instant) -> Result<String, String> {
        let id = crate::web::token::generate()?;
        self.sweep(now);
        if self.sessions.len() >= SESSION_MAX {
            self.evict_oldest_session();
        }
        self.sessions.insert(id.clone(), (via, now));
        Ok(id)
    }

    fn evict_oldest_session(&mut self) {
        let oldest = self
            .sessions
            .iter()
            .min_by_key(|(_, (_, born))| *born)
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            self.sessions.remove(&id);
        }
    }

    pub(crate) fn session(&self, id: &str, now: Instant) -> Option<Via> {
        let (via, born) = self.sessions.get(id)?;
        alive(*born, SESSION_TTL, now).then_some(*via)
    }

    pub(crate) fn close_session(&mut self, id: &str) {
        self.sessions.remove(id);
    }

    /// Clears sessions only; temporary tokens live until expiry, `take_temp` or `drop_temp_tokens`.
    pub(crate) fn close_sessions_except(&mut self, keep_id: Option<&str>) {
        self.sessions.retain(|id, _| Some(id.as_str()) == keep_id);
    }

    pub(crate) fn drop_temp_tokens(&mut self) {
        self.temps.clear();
    }
}

#[cfg(test)]
#[path = "tickets_tests.rs"]
mod tests;
