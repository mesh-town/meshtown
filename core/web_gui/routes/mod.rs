//! Router of the node page: registers routes by group (the static ones from the `pages` table, the
//! network routes under two prefixes and the process routes once), then the shared `fallback` (404) and a
//! layer of headers and guards. Handlers are in neighboring files by group (`pages`, `state`, `config`,
//! `peers`, `domains`, `actions`, `network`, `nets`, `overview`, `auth`, `auth_password`, `token_rotate`), shared helpers are in
//! `helpers.rs`, the slot a request addresses is read by `slot_param`.

use std::sync::Arc;

use axum::http::{header, HeaderName, HeaderValue};
use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::set_header::SetResponseHeaderLayer;

use super::http::{body_limit, reject_chunked};
use super::{guard, App};

mod actions;
mod auth;
mod auth_password;
mod config;
mod domains;
pub(crate) mod helpers;
mod inspect;
#[cfg(test)]
pub(crate) mod invite_fixture;
mod join_target;
mod log;
pub(crate) mod nets;
mod network;
mod network_invite;
mod overview;
mod pages;
mod peers;
pub(crate) mod slot_param;
mod state;
mod state_host;
mod state_net;
mod token_rotate;

use helpers::{bad_method, fallback_404, reject_mutations_while_stopping};

const CSP: &str =
    "default-src 'none'; style-src 'self'; script-src 'self'; connect-src 'self' https://api.github.com; img-src data:; form-action 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Builds the router of the node page: routes by group, then the shared `fallback` (404) and a layer
/// of headers and guards (order, outside to inside: headers → guard::guard_middleware →
/// reject_chunked → body_limit → reject_mutations_while_stopping → handler).
pub(crate) fn router(app: Arc<App>) -> Router {
    let router = Router::new();
    let router = register_pages(router);
    let router = register_net_routes(router, "/api");
    let router = register_net_routes(router, "/api/net/{slot}");
    let router = register_process(router);
    let router = register_auth(router);
    let router = register_token(router);
    let router = register_host(router);
    router
        .fallback(fallback_404)
        .layer(
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-content-type-options"),
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("content-security-policy"),
                    HeaderValue::from_static(CSP),
                ))
                .layer(middleware::from_fn_with_state(
                    app.clone(),
                    guard::guard_middleware,
                ))
                .layer(middleware::from_fn(reject_chunked))
                .layer(middleware::from_fn(body_limit))
                .layer(middleware::from_fn_with_state(
                    app.clone(),
                    reject_mutations_while_stopping,
                )),
        )
        .with_state(app)
}

fn register_pages(router: Router<Arc<App>>) -> Router<Arc<App>> {
    pages::register(router)
}

/// Registers the routes of one network under `base`: `/api` addresses the primary slot, `/api/net/{slot}`
/// any slot. Most handlers serve both (`SlotParam` reads the prefix); `join` differs, see `register_network`.
pub(crate) fn register_net_routes(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    let router = router.route(
        &format!("{base}/state"),
        get(state::state_handler).fallback(bad_method),
    );
    let router = register_config(router, base);
    let router = register_peers(router, base);
    let router = register_domains(router, base);
    let router = register_actions(router, base);
    let router = register_network(router, base);
    crate::web_gui::members::register_members(router, base)
}

/// The list of networks and the overview of all of them: process routes with their own paths.
fn register_host(router: Router<Arc<App>>) -> Router<Arc<App>> {
    router
        .route(
            "/api/nets",
            get(nets::nets_get_handler)
                .post(nets::nets_post_handler)
                .fallback(bad_method),
        )
        .route(
            "/api/overview",
            get(overview::overview_handler).fallback(bad_method),
        )
        .route(
            "/api/notices/ack",
            post(state_host::notices_ack_handler).fallback(bad_method),
        )
}

/// Routes of the process: registered once, without a slot prefix.
fn register_process(router: Router<Arc<App>>) -> Router<Arc<App>> {
    router
        .route("/api/log", get(log::log_handler).fallback(bad_method))
        .route(
            "/api/log/download",
            get(log::log_download_handler).fallback(bad_method),
        )
        .route(
            "/api/actions/stop",
            post(actions::action_stop_handler).fallback(bad_method),
        )
}

fn register_config(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    router.route(
        &format!("{base}/config"),
        get(config::config_get_handler)
            .put(config::config_put_handler)
            .fallback(bad_method),
    )
}

fn register_peers(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    router
        .route(
            &format!("{base}/peer-addresses"),
            get(peers::peer_addresses_get_handler)
                .post(peers::peer_addresses_post_handler)
                .fallback(bad_method),
        )
        .route(
            &format!("{base}/network/key"),
            get(peers::network_key_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/network/invite"),
            get(peers::network_invite_handler).fallback(bad_method),
        )
}

fn register_domains(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    router.route(
        &format!("{base}/domains"),
        post(domains::domains_post_handler)
            .delete(domains::domains_delete_handler)
            .fallback(bad_method),
    )
}

fn register_actions(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    router
        .route(
            &format!("{base}/actions/find-exit"),
            post(actions::action_find_exit_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/actions/check-domain"),
            post(actions::action_check_domain_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/actions/check-all"),
            post(actions::action_check_all_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/actions/restart"),
            post(actions::action_restart_handler).fallback(bad_method),
        )
}

/// `/api/network/join` without a slot picks the slot for an invite itself; `/api/net/{slot}/network/join`
/// joins the slot it names.
fn register_network(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    let join = match base == "/api" {
        true => post(join_target::network_join_auto_handler),
        false => post(network::network_join_handler),
    };
    router
        .route(&format!("{base}/network/join"), join.fallback(bad_method))
        .route(
            &format!("{base}/network/create"),
            post(network::network_create_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/network/inspect"),
            post(inspect::network_inspect_handler).fallback(bad_method),
        )
}

fn register_auth(router: Router<Arc<App>>) -> Router<Arc<App>> {
    router
        .route(
            "/api/auth/status",
            get(auth::status_handler).fallback(bad_method),
        )
        .route(
            "/api/auth/token",
            post(auth::token_handler).fallback(bad_method),
        )
        .route(
            "/api/auth/login",
            post(auth::login_handler).fallback(bad_method),
        )
        .route(
            "/api/auth/logout",
            post(auth::logout_handler).fallback(bad_method),
        )
        .route(
            "/api/auth/reset",
            post(auth::reset_handler).fallback(bad_method),
        )
        .route(
            "/api/auth/password",
            put(auth_password::password_put_handler).fallback(bad_method),
        )
}

fn register_token(router: Router<Arc<App>>) -> Router<Arc<App>> {
    router.route(
        "/api/token/rotate",
        post(token_rotate::token_rotate_handler).fallback(bad_method),
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "net_routes_tests.rs"]
mod net_routes_tests;
