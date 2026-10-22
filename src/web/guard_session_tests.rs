//! Tests for `guard.rs`: sessions, the temporary token, open sign-in handlers, `admit_request`.

use super::tests::*;
use super::*;
use axum::http::Request as HttpRequest;

mod session {
    use super::*;

    fn with_cookie<'a>(name: &str, id: &str, extra: &'a [(&'a str, &'a str)]) -> Parts {
        let line = format!("{name}={id}");
        let host = lo_host();
        let mut h = vec![("host", host.as_str()), ("cookie", line.as_str())];
        h.extend_from_slice(extra);
        get("/api/state", &h)
    }

    #[test]
    fn valid_cookie_gives_session_credential() {
        let s = shared(ListenAccess::Localhost);
        let id = open_session(&s);
        let p = with_cookie(COOKIE, &id, &[]);
        let cred = check_request(&p, &lo(), &s).unwrap().unwrap();
        assert_eq!(cred.via, Via::Password);
        assert_eq!(cred.session_id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn closed_is_401() {
        let s = shared(ListenAccess::Localhost);
        let id = open_session(&s);
        s.auth.tickets.lock().unwrap().close_session(&id);
        let err = check_request(&with_cookie(COOKIE, &id, &[]), &lo(), &s).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn other_node_cookie_name_is_401() {
        let s = shared(ListenAccess::Localhost);
        let id = open_session(&s);
        let p = with_cookie("mesh_other00000", &id, &[]);
        let err = check_request(&p, &lo(), &s).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn dead_cookie_with_token_falls_back_to_token() {
        let s = shared(ListenAccess::Localhost);
        let p = with_cookie(COOKIE, TOKEN, &[("x-meshtown-token", TOKEN)]);
        let cred = check_request(&p, &lo(), &s).unwrap().unwrap();
        assert_eq!(cred.via, Via::PermanentToken);
        assert!(cred.session_id.is_none());
    }
}

mod temp_token {
    use super::*;

    fn issue(s: &WebShared) -> String {
        s.auth.tickets.lock().unwrap().issue_temp(Instant::now()).unwrap()
    }

    #[test]
    fn in_header_is_401() {
        let s = shared(ListenAccess::Localhost);
        let t = issue(&s);
        let p = get("/api/state", &[("host", &lo_host()), ("x-meshtown-token", &t)]);
        let err = check_request(&p, &lo(), &s).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn in_query_is_401() {
        let s = shared(ListenAccess::Localhost);
        let t = issue(&s);
        let p = get(&format!("/api/state?token={t}"), &[("host", &lo_host())]);
        let err = check_request(&p, &lo(), &s).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }
}

mod open_api {
    use super::*;

    fn bare(method: &str, path: &str) -> Result<Option<Credential>, Response> {
        let origin = format!("http://{}", lo_host());
        let p = parts(method, path, &[("host", &lo_host()), ("origin", &origin)]);
        check_request(&p, &lo(), &shared(ListenAccess::Localhost))
    }

    #[test]
    fn status_without_credentials_is_ok_none() {
        assert!(bare("GET", "/api/auth/status").unwrap().is_none());
    }

    #[test]
    fn token_without_credentials_is_ok_none() {
        assert!(bare("POST", "/api/auth/token").unwrap().is_none());
    }

    #[test]
    fn login_without_credentials_is_ok_none() {
        assert!(bare("POST", "/api/auth/login").unwrap().is_none());
    }

    #[test]
    fn logout_without_credentials_is_401() {
        let err = bare("POST", "/api/auth/logout").unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn status_with_session_carries_credential() {
        let s = shared(ListenAccess::Localhost);
        let id = open_session(&s);
        let line = format!("{COOKIE}={id}");
        let p = get("/api/auth/status", &[("host", &lo_host()), ("cookie", &line)]);
        let cred = check_request(&p, &lo(), &s).unwrap().unwrap();
        assert_eq!(cred.session_id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn status_prefix_trick_is_401() {
        let err = bare("GET", "/api/auth/statusX").unwrap_err();
        assert_eq!(code_of(&err), StatusCode::UNAUTHORIZED);
    }
}

mod admit {
    use super::*;
    use axum::body::Body;

    fn request(headers: &[(&str, &str)]) -> Request {
        let mut b = HttpRequest::builder().uri("/api/state");
        for (name, value) in headers {
            b = b.header(*name, *value);
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn without_conn_info_is_403_bad_peer() {
        let req = request(&[("host", &lo_host()), ("x-meshtown-token", TOKEN)]);
        let err = admit_request(req, &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::FORBIDDEN);
        assert_eq!(error_code(err).await, "bad_peer");
    }

    #[test]
    fn with_token_puts_credential_into_extensions() {
        let mut req = request(&[("host", &lo_host()), ("x-meshtown-token", TOKEN)]);
        req.extensions_mut().insert(lo());
        let out = admit_request(req, &shared(ListenAccess::Localhost)).unwrap();
        let cred = out.extensions().get::<Credential>().unwrap();
        assert_eq!(cred.via, Via::PermanentToken);
    }
}
