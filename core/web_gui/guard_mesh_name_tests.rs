//! Tests for the `<label>.meshtown` form of `Host` and `Origin` in `guard.rs`.

use super::tests::{code_of, conn, error_code, get, lo, parts, shared, PORT};
use super::*;

const NAME: &str = "node-1.meshtown";

fn named() -> String {
    format!("{NAME}:{PORT}")
}

#[test]
fn loopback_client_may_use_mesh_name_with_port() {
    let p = get("/api/auth/status", &[("host", &named())]);
    assert!(check_request(&p, &lo(), &shared(ListenAccess::Localhost)).is_ok());
}

#[test]
fn loopback_client_may_use_mesh_name_without_port() {
    let p = get("/api/auth/status", &[("host", NAME)]);
    assert!(check_request(&p, &lo(), &shared(ListenAccess::Localhost)).is_ok());
}

#[test]
fn mesh_name_is_case_insensitive() {
    let p = get("/api/auth/status", &[("host", "Node-1.MeshTown")]);
    assert!(check_request(&p, &lo(), &shared(ListenAccess::Localhost)).is_ok());
}

#[tokio::test]
async fn foreign_names_are_bad_host_even_from_loopback() {
    for host in [
        "localhost",
        "x.meshtown.evil",
        "a.b.meshtown",
        "node.meshtown:0",
        "node.meshtown:70000",
    ] {
        let p = get("/api/auth/status", &[("host", host)]);
        let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
        assert_eq!(code_of(&err), StatusCode::FORBIDDEN, "{host}");
        assert_eq!(error_code(err).await, "bad_host", "{host}");
    }
}

#[tokio::test]
async fn non_loopback_client_gets_bad_host_for_mesh_name() {
    let c = conn("192.168.1.5:50000", &format!("192.168.1.9:{PORT}"));
    let p = get("/api/auth/status", &[("host", &named())]);
    let err = check_request(&p, &c, &shared(ListenAccess::Localnet)).unwrap_err();
    assert_eq!(error_code(err).await, "bad_host");
}

#[tokio::test]
async fn second_host_value_with_mesh_name_is_bad_host() {
    let p = get(
        "/api/auth/status",
        &[("host", &named()), ("host", &named())],
    );
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(error_code(err).await, "bad_host");
}

#[test]
fn origin_with_the_accepted_name_passes() {
    let origin = format!("http://{}", named());
    let p = parts(
        "POST",
        "/api/auth/login",
        &[("host", &named()), ("origin", &origin)],
    );
    assert!(check_request(&p, &lo(), &shared(ListenAccess::Localhost)).is_ok());
}

#[tokio::test]
async fn origin_literal_does_not_match_mesh_name_host() {
    let origin = format!("http://127.0.0.1:{PORT}");
    let p = parts(
        "POST",
        "/api/auth/login",
        &[("host", &named()), ("origin", &origin)],
    );
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(error_code(err).await, "bad_origin");
}

#[tokio::test]
async fn post_without_origin_on_mesh_name_is_bad_origin() {
    let p = parts("POST", "/api/auth/login", &[("host", &named())]);
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(error_code(err).await, "bad_origin");
}

#[tokio::test]
async fn absolute_form_with_mesh_name_is_bad_host() {
    let uri = format!("http://{}/api/auth/status", named());
    let p = get(&uri, &[("host", &named())]);
    let err = check_request(&p, &lo(), &shared(ListenAccess::Localhost)).unwrap_err();
    assert_eq!(error_code(err).await, "bad_host");
}
