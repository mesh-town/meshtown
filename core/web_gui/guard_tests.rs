//! Tests for `guard.rs`: peer → target → `Host` → `Origin` → credentials, sessions and open handlers.

use super::*;
use crate::web_gui::auth::state::AuthState;
use axum::http::Request as HttpRequest;

pub(super) const PORT: u16 = 18765;
pub(super) const TOKEN: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";
pub(super) const COOKIE: &str = "mesh_abcdef0123";

pub(super) fn shared(mode: ListenAccess) -> WebShared {
    shared_named(mode, COOKIE)
}

pub(super) fn shared_named(mode: ListenAccess, cookie: &str) -> WebShared {
    let auth = AuthState::new(cookie.to_string(), None, mode);
    WebShared::new(PORT, TOKEN.to_string(), auth)
}

pub(super) fn conn(peer: &str, local: &str) -> ConnInfo {
    ConnInfo {
        client_addr: peer.parse().unwrap(),
        local: local.parse().unwrap(),
    }
}

pub(super) fn lo() -> ConnInfo {
    ConnInfo::loopback(PORT)
}

pub(super) fn lo_host() -> String {
    format!("127.0.0.1:{PORT}")
}

pub(super) fn parts(method: &str, path: &str, headers: &[(&str, &str)]) -> Parts {
    let mut b = HttpRequest::builder().method(method).uri(path);
    for (name, value) in headers {
        b = b.header(*name, *value);
    }
    b.body(()).unwrap().into_parts().0
}

pub(super) fn get(path: &str, headers: &[(&str, &str)]) -> Parts {
    parts("GET", path, headers)
}

pub(super) fn code_of(resp: &Response) -> StatusCode {
    resp.status()
}

pub(super) async fn error_code(resp: Response) -> String {
    use http_body_util::BodyExt;
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["error"].as_str().unwrap().to_string()
}

pub(super) async fn error_message(resp: Response) -> String {
    use http_body_util::BodyExt;
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["message"].as_str().unwrap().to_string()
}

pub(super) fn open_session(s: &WebShared) -> String {
    let mut t = s.auth.tickets.lock().unwrap();
    t.open_session(Via::Password, Instant::now()).unwrap()
}

#[test]
fn wrong_host_without_token_is_403_not_401() {
    let p = get("/api/state", &[("host", "evil.example")]);
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn duplicate_host_header_is_403() {
    let p = get(
        "/api/state",
        &[("host", &lo_host()), ("host", "evil.example")],
    );
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn post_without_origin_is_403_bad_origin() {
    let p = parts("POST", "/api/state", &[("host", &lo_host())]);
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn get_without_origin_with_valid_token_is_ok() {
    let p = get(
        "/api/state",
        &[("host", &lo_host()), ("x-meshtown-token", TOKEN)],
    );
    let ok = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap();
    assert!(ok.is_some());
}

#[test]
fn get_with_wrong_origin_is_403_bad_origin() {
    let p = get(
        "/api/state",
        &[("host", &lo_host()), ("origin", "http://evil.example")],
    );
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn static_asset_without_token_is_ok() {
    let p = get("/app.css", &[("host", &lo_host())]);
    let ok = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap();
    assert!(ok.is_none());
}

#[test]
fn token_in_header_is_ok() {
    let p = get(
        "/api/state",
        &[("host", &lo_host()), ("x-meshtown-token", TOKEN)],
    );
    let cred = check_request(&p, &lo(), &shared(ListenAccess::Localhost))
        .unwrap()
        .unwrap();
    assert_eq!(cred.via, Via::PermanentToken);
    assert!(cred.session_id.is_none());
}

#[test]
fn token_in_query_is_ok() {
    let p = get(
        &format!("/api/state?token={TOKEN}"),
        &[("host", &lo_host())],
    );
    let ok = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap();
    assert!(ok.is_some());
}

#[test]
fn wrong_token_is_401() {
    let p = get(
        "/api/state",
        &[("host", &lo_host()), ("x-meshtown-token", "wrong")],
    );
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
}

#[test]
fn port_80_expects_host_without_port() {
    let s = shared(ListenAccess::Localhost);
    let c = ConnInfo::loopback(80);
    let p = get("/app.css", &[("host", "127.0.0.1")]);
    assert!(check_request(&p, &c, &s).is_ok());
    let with_port = get("/app.css", &[("host", "127.0.0.1:80")]);
    let err = check_request(&with_port, &c, &s).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn absolute_form_target_is_403_even_with_correct_host() {
    let p = get("http://evil.example/api/state", &[("host", &lo_host())]);
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
}

#[test]
fn ct_eq_behaves() {
    assert!(ct_eq(b"abc", b"abc"));
    assert!(!ct_eq(b"abc", b"abd"));
    assert!(!ct_eq(b"abc", b"ab"));
}

mod localhost {
    use super::*;

    fn lan_conn() -> ConnInfo {
        conn("192.168.1.9:50000", &format!("127.0.0.1:{PORT}"))
    }

    #[tokio::test]
    async fn lan_peer_is_403_bad_peer() {
        let p = get("/app.css", &[("host", &lo_host())]);
        let err = check_request(&p, &lan_conn(), &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
        assert_eq!(error_code(err).await, "bad_peer");
    }

    #[tokio::test]
    async fn lan_peer_message_names_the_page() {
        let p = get("/app.css", &[("host", &lo_host())]);
        let err = check_request(&p, &lan_conn(), &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(
            error_message(err).await,
            "the Mesh Town page is open only from this computer"
        );
    }

    #[tokio::test]
    async fn public_peer_message_names_the_local_network() {
        let p = get("/app.css", &[("host", &lo_host())]);
        let public = conn("8.8.8.8:50000", &format!("127.0.0.1:{PORT}"));
        let err = check_request(&p, &public, &shared(ListenAccess::Localnet)).unwrap_err();
        assert_eq!(
            error_message(err).await,
            "the Mesh Town page is open only from the local network"
        );
    }

    #[tokio::test]
    async fn lan_peer_with_wrong_host_bad_peer_first() {
        let p = get("http://evil.example/x", &[("host", "evil.example")]);
        let err = check_request(&p, &lan_conn(), &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(error_code(err).await, "bad_peer");
    }
}

mod localnet {
    use super::*;

    fn lan_local() -> String {
        format!("192.168.1.5:{PORT}")
    }

    #[tokio::test]
    async fn lan_peer_host_must_be_local_ip() {
        let c = conn("192.168.1.9:50000", &lan_local());
        let s = shared(ListenAccess::Localnet);
        let ok = get("/app.css", &[("host", &lan_local())]);
        assert!(check_request(&ok, &c, &s).is_ok());
        let bad = get("/app.css", &[("host", &lo_host())]);
        let err = check_request(&bad, &c, &s).unwrap_err();
        assert_eq!(error_code(err).await, "bad_host");
    }

    #[tokio::test]
    async fn cgnat_peer_is_403_bad_peer() {
        let c = conn("100.64.0.7:50000", &lan_local());
        let p = get("/app.css", &[("host", &lan_local())]);
        let err = check_request(&p, &c, &shared(ListenAccess::Localnet)).unwrap_err();
        assert_eq!(error_code(err).await, "bad_peer");
    }

    #[tokio::test]
    async fn public_peer_is_403_bad_peer() {
        let c = conn("8.8.8.8:50000", &lan_local());
        let p = get("/app.css", &[("host", &lan_local())]);
        let err = check_request(&p, &c, &shared(ListenAccess::Localnet)).unwrap_err();
        assert_eq!(error_code(err).await, "bad_peer");
    }

    #[tokio::test]
    async fn host_is_domain_is_403_bad_host() {
        let c = conn("192.168.1.9:50000", &lan_local());
        let p = get("/app.css", &[("host", "node.local")]);
        let err = check_request(&p, &c, &shared(ListenAccess::Localnet)).unwrap_err();
        assert_eq!(error_code(err).await, "bad_host");
    }
}

mod narrowed {
    use super::*;

    #[tokio::test]
    async fn lan_peer_after_set_access_localhost_is_403_bad_peer() {
        let s = shared(ListenAccess::Localnet);
        let c = conn("192.168.1.9:50000", &format!("192.168.1.5:{PORT}"));
        let p = get("/app.css", &[("host", &format!("192.168.1.5:{PORT}"))]);
        assert!(check_request(&p, &c, &s).is_ok());
        s.set_access(ListenAccess::Localhost);
        let err = check_request(&p, &c, &s).unwrap_err();
        assert_eq!(error_code(err).await, "bad_peer");
    }
}

mod origin {
    use super::*;

    #[test]
    fn port_80_expects_origin_without_port() {
        let s = shared(ListenAccess::Localhost);
        let c = ConnInfo::loopback(80);
        let ok = parts(
            "POST",
            "/api/auth/login",
            &[("host", "127.0.0.1"), ("origin", "http://127.0.0.1")],
        );
        assert!(check_request(&ok, &c, &s).is_ok());
        let with_port = parts(
            "POST",
            "/api/auth/login",
            &[("host", "127.0.0.1"), ("origin", "http://127.0.0.1:80")],
        );
        assert!(check_request(&with_port, &c, &s).is_err());
    }

    #[test]
    fn lan_post_with_lan_origin_is_ok() {
        let host = format!("192.168.1.5:{PORT}");
        let c = conn("192.168.1.9:50000", &host);
        let origin = format!("http://{host}");
        let p = parts(
            "POST",
            "/api/auth/login",
            &[("host", &host), ("origin", &origin)],
        );
        assert!(check_request(&p, &c, &shared(ListenAccess::Localnet)).is_ok());
    }

    #[tokio::test]
    async fn patch_without_origin_is_403_bad_origin() {
        let p = parts(
            "PATCH",
            "/api/config",
            &[("host", &lo_host()), ("x-meshtown-token", TOKEN)],
        );
        let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(
            error_message(err).await,
            "the request didn't come from the Mesh Town page"
        );
    }
}

mod iface_gate {
    use super::*;
    use crate::config::config_file::IfaceFilter;

    #[tokio::test]
    async fn unlisted_iface_is_403_bad_peer() {
        let sh = shared(ListenAccess::Localnet);
        sh.set_ifaces(IfaceFilter::Only(vec!["no-such0".to_string()]));
        let c = conn("192.168.1.9:50000", &format!("192.168.1.10:{PORT}"));
        let p = get("/app.css", &[("host", &host_of(c.local.ip(), PORT))]);
        let err = check_request(&p, &c, &sh).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
        assert_eq!(error_code(err).await, "bad_peer");
    }
}
