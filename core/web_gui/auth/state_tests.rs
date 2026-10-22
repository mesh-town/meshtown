//! Tests for `state.rs`: sign-in URLs, temporary tokens, access mode.

use super::*;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Instant;

fn temp_dir() -> PathBuf {
    // nextest runs each test in a separate process, and pids are reused: a "pid + counter" directory
    // could be left over from an earlier process with the same pid together with its password file, and a test without a password
    // got a URL without `?token=` (the red `no_password::one_url_with_token`).
    // A time in the name makes the directory new for any pid; just in case, we clean it too.
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mesh-auth-state-{}-{nanos}-{seq}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A password file made by hand per the "On-disk format" section: a fast hash is not needed, the check is "is it set".
fn write_password(dir: &std::path::Path, mode: u32) {
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let json = format!(
        "{{\"version\":1,\"alg\":\"pbkdf2-sha256\",\"iterations\":1000,\"salt\":\"{}\",\"hash\":\"{}\",\"set_at\":1}}\n",
        b64(&[7u8; 16]),
        b64(&[9u8; 32])
    );
    let path = dir.join(super::super::password_file::PASSWORD_FILE_NAME);
    std::fs::write(&path, json).unwrap();
    set_mode(&path, mode);
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(not(unix))]
fn set_mode(_path: &std::path::Path, _mode: u32) {}

fn state(dir: Option<PathBuf>) -> AuthState {
    AuthState::new("mesh_test".to_string(), dir, ListenAccess::Localhost)
}

fn token_of(url: &str) -> String {
    url.split("?token=").nth(1).unwrap().to_string()
}

mod no_password {
    use super::*;

    #[test]
    fn one_url_with_token() {
        let st = state(Some(temp_dir()));
        let urls = st.issue_entry_urls(7070);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].starts_with("http://127.0.0.1:7070/?token="));
        assert_eq!(token_of(&urls[0]).len(), 52);
    }

    #[test]
    fn each_call_new_token() {
        let st = state(Some(temp_dir()));
        let a = token_of(&st.issue_entry_urls(7070)[0]);
        let b = token_of(&st.issue_entry_urls(7070)[0]);
        assert_ne!(a, b);
    }

    #[test]
    fn token_taken_once() {
        let st = state(Some(temp_dir()));
        let t = token_of(&st.issue_entry_urls(7070)[0]);
        let mut tickets = st.tickets.lock().unwrap();
        assert!(tickets.take_temp(&t, Instant::now()));
        assert!(!tickets.take_temp(&t, Instant::now()));
    }
}

mod password_set {
    use super::*;

    #[test]
    fn urls_without_token_and_no_temp_issued() {
        let dir = temp_dir();
        write_password(&dir, 0o600);
        let st = state(Some(dir));
        assert!(st.password_set());
        assert_eq!(
            st.issue_entry_urls(7070),
            vec!["http://127.0.0.1:7070/".to_string()]
        );
        let mut tickets = st.tickets.lock().unwrap();
        assert!(!tickets.take_temp("0".repeat(52).as_str(), Instant::now()));
    }
}

#[cfg(unix)]
mod unreadable_file {
    use super::*;

    #[test]
    fn treated_as_no_password() {
        let dir = temp_dir();
        write_password(&dir, 0o644);
        let st = state(Some(dir));
        assert!(!st.password_set());
        assert!(st.issue_entry_urls(7070)[0].contains("?token="));
    }
}

mod no_dir {
    use super::*;

    #[test]
    fn url_with_token() {
        let st = state(None);
        assert!(!st.password_set());
        assert!(st.issue_entry_urls(7070)[0].contains("?token="));
    }
}

mod port_80 {
    use super::*;

    #[test]
    fn url_without_port() {
        let dir = temp_dir();
        write_password(&dir, 0o600);
        let st = state(Some(dir));
        assert_eq!(
            st.issue_entry_urls(80),
            vec!["http://127.0.0.1/".to_string()]
        );
    }
}

mod url_hosts {
    use super::*;
    use ListenAccess::{Localhost, Localnet};

    const LAN: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 5);
    const LOOP: Ipv4Addr = Ipv4Addr::LOCALHOST;

    #[test]
    fn both_localnet_with_lan_gives_two() {
        assert_eq!(url_hosts(Localnet, Localnet, Some(LAN)), vec![LOOP, LAN]);
    }

    #[test]
    fn access_narrowed_gives_one() {
        assert_eq!(url_hosts(Localnet, Localhost, Some(LAN)), vec![LOOP]);
    }

    #[test]
    fn listening_localhost_gives_one() {
        assert_eq!(url_hosts(Localhost, Localnet, Some(LAN)), vec![LOOP]);
    }

    #[test]
    fn cgnat_lan_ignored() {
        let cgnat = Ipv4Addr::new(100, 64, 0, 5);
        assert_eq!(url_hosts(Localnet, Localnet, Some(cgnat)), vec![LOOP]);
    }

    #[test]
    fn no_lan_gives_one() {
        assert_eq!(url_hosts(Localnet, Localnet, None), vec![LOOP]);
    }
}

mod set_access {
    use super::*;

    #[test]
    fn changes_access_not_listening() {
        let st = AuthState::new("mesh_test".to_string(), None, ListenAccess::Localhost);
        st.set_access(ListenAccess::Localnet);
        assert_eq!(st.access(), ListenAccess::Localnet);
        assert_eq!(st.bound_access(), ListenAccess::Localhost);
    }
}

mod ifaces {
    use super::*;
    use crate::web_gui::auth::iface_os::view_from;
    use crate::web_gui::http::ConnInfo;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    fn row(name: &str, addr: &str) -> (String, Ipv4Addr, Ipv4Addr, bool) {
        (name.to_string(), ip(addr), ip("255.255.255.0"), true)
    }

    fn view() -> IfaceView {
        let rows = vec![row("lan0", "192.168.88.1"), row("wan0", "192.168.1.100")];
        view_from(rows, Some(ip("192.168.1.100")), true)
    }

    fn conn(peer: &str, local: &str) -> ConnInfo {
        ConnInfo {
            client_addr: peer.parse().unwrap(),
            local: local.parse().unwrap(),
        }
    }

    fn state(mode: ListenAccess) -> AuthState {
        AuthState::new("mesh_test".to_string(), None, mode)
    }

    #[test]
    fn set_and_get() {
        let st = state(ListenAccess::Localnet);
        assert_eq!(st.ifaces(), IfaceFilter::Auto);
        st.set_ifaces(IfaceFilter::Any);
        assert_eq!(st.ifaces(), IfaceFilter::Any);
    }

    #[test]
    fn mode_follows_listening_localhost() {
        let st = state(ListenAccess::Localhost);
        st.set_access(ListenAccess::Localnet);
        assert_eq!(st.mode(), ListenAccess::Localhost);
        assert_eq!(state(ListenAccess::Localnet).mode(), ListenAccess::Localnet);
    }

    #[test]
    fn verdict_by_ingress() {
        let st = state(ListenAccess::Localnet);
        st.set_ifaces(IfaceFilter::Only(vec!["lan0".to_string()]));
        let v = view();
        let from_lan = conn("10.9.9.9:5000", "192.168.88.1:7070");
        let from_wan = conn("10.9.9.9:5000", "192.168.1.100:7070");
        let from_loop = conn("127.0.0.1:5000", "127.0.0.1:7070");
        assert!(st.iface_verdict(&v, &from_lan).allows());
        assert!(!st.iface_verdict(&v, &from_wan).allows());
        assert!(st.iface_verdict(&v, &from_loop).allows());
    }
}
