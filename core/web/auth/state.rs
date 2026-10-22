//! Sign-in state of one process: tables of sessions and tokens, the limiter,
//! the access mode. Lives in `WebShared.auth` from `start` to the end of the process; a restart gives a clean
//! table.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use super::access::{lan_addrs, is_client_allowed};
use super::iface_gate::{judge, IfaceView, Verdict};
use super::iface_nets::admitted_lan;
use super::iface_os::current_view;
use super::limiter::Limiter;
use super::password_file::{read_password_file, PasswordFileState};
use super::tickets::Tickets;
use crate::config_file::{IfaceFilter, ListenAccess};
use crate::web::http::{host_of, ConnInfo};

static UNREADABLE_LOGGED: AtomicBool = AtomicBool::new(false);

pub(crate) struct AuthState {
    pub(crate) tickets: Mutex<Tickets>,
    pub(crate) limiter: Mutex<Limiter>,
    /// One password check at a time; taken by the password-check route.
    pub(crate) hash_gate: tokio::sync::Semaphore,
    pub(crate) cookie_name: String,
    pub(crate) password_dir: Option<PathBuf>,
    access: Mutex<ListenAccess>,
    ifaces: Mutex<IfaceFilter>,
    bound_access: ListenAccess,
}

/// Hosts to print: loopback always, the network address only if it will open.
fn url_hosts(bound_access: ListenAccess, access: ListenAccess, lan: Option<Ipv4Addr>) -> Vec<Ipv4Addr> {
    let mut hosts = vec![Ipv4Addr::LOCALHOST];
    let both_net = bound_access == ListenAccess::Localnet && access == ListenAccess::Localnet;
    let usable = lan.filter(|ip| both_net && is_client_allowed(ListenAccess::Localnet, (*ip).into()));
    hosts.extend(usable);
    hosts
}

fn log_unreadable_once(why: &str) {
    if !UNREADABLE_LOGGED.swap(true, Ordering::Relaxed) {
        crate::output::say(
            "web",
            format!("password file can't be read ({why}): sign-in by token only"),
        );
    }
}

impl AuthState {
    pub(crate) fn new(
        cookie_name: String,
        password_dir: Option<PathBuf>,
        mode: ListenAccess,
    ) -> AuthState {
        AuthState {
            tickets: Mutex::new(Tickets::new()),
            limiter: Mutex::new(Limiter::new()),
            hash_gate: tokio::sync::Semaphore::new(1),
            cookie_name,
            password_dir,
            access: Mutex::new(mode),
            ifaces: Mutex::new(IfaceFilter::Auto),
            bound_access: mode,
        }
    }

    pub(crate) fn access(&self) -> ListenAccess {
        *self.access.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn bound_access(&self) -> ListenAccess {
        self.bound_access
    }

    /// Only remembers: a widened mode takes effect after a restart.
    pub(crate) fn set_access(&self, mode: ListenAccess) {
        *self.access.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    }

    /// The effective interface filter of the page.
    pub(crate) fn ifaces(&self) -> IfaceFilter {
        self.ifaces.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Takes effect immediately, both narrowing and widening (the listener is on all addresses anyway).
    pub(crate) fn set_ifaces(&self, filter: IfaceFilter) {
        *self.ifaces.lock().unwrap_or_else(|e| e.into_inner()) = filter;
    }

    /// The mode used to judge a connection: a listener on loopback is always `Localhost`.
    pub(crate) fn mode(&self) -> ListenAccess {
        match self.bound_access {
            ListenAccess::Localhost => ListenAccess::Localhost,
            ListenAccess::Localnet => self.access(),
        }
    }

    /// The interface verdict for an accepted connection (on top of `is_client_allowed`).
    pub(crate) fn iface_verdict(&self, view: &IfaceView, conn: &ConnInfo) -> Verdict {
        let local = Some(conn.local.ip());
        judge(self.mode(), &self.ifaces(), view, conn.client_addr.ip(), local)
    }

    /// The first address in a network open to the page.
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        let view = current_view();
        let open = admitted_lan(self.mode(), &self.ifaces(), &view, lan_addrs());
        open.first().map(|(_, ip)| *ip)
    }

    /// The file is read every time; an unreadable one means "no password", one line in the log.
    pub(crate) fn password_set(&self) -> bool {
        let Some(dir) = &self.password_dir else {
            return false;
        };
        match read_password_file(dir) {
            PasswordFileState::Set(_) => true,
            PasswordFileState::Absent => false,
            PasswordFileState::Unreadable(why) => {
                log_unreadable_once(&why);
                false
            }
        }
    }

    /// Sign-in URLs. No password — with a new temporary token (one for all addresses).
    pub(crate) fn issue_entry_urls(&self, port: u16) -> Vec<String> {
        let hosts = url_hosts(self.bound_access, self.access(), self.lan_ip());
        let suffix = self.entry_suffix();
        let url = |ip: &Ipv4Addr| format!("http://{}/{suffix}", host_of((*ip).into(), port));
        hosts.iter().map(url).collect()
    }

    fn entry_suffix(&self) -> String {
        if self.password_set() {
            return String::new();
        }
        let issued = self
            .tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .issue_temp(Instant::now());
        match issued {
            Ok(t) => format!("?token={t}"),
            Err(e) => {
                crate::output::say("web", format!("temporary token not issued: {e}"));
                String::new()
            }
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
