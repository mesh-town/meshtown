//! Starts and stops the page listener: `start_web_page`, `accept_loop`, `WebHandle::shutdown`.

use std::sync::Arc;

use super::auth::access::{bind_ip, is_client_allowed};
use super::auth::iface_os::current_view;
use super::auth::state::AuthState;
use super::http::ConnInfo;
use super::{cookie, token, App, HostControl, WebHandle, WebOptions, WebShared, WebStartError};
use crate::config::config_file::ListenAccess;
use crate::node::output::NET_NAME;

/// Starts the node page: binds the listener, prints the announce line and spawns the accept loop.
/// The token is never printed anywhere —
/// the log line at the final step doesn't contain it.
pub async fn start_web_page(
    opts: WebOptions,
    run_config: &crate::config::config_file::RunConfig,
    host: Arc<dyn HostControl>,
) -> Result<WebHandle, WebStartError> {
    if !opts.enabled {
        return Err(WebStartError::Disabled);
    }

    let stored = stored_token_before_bind(&opts)?;

    let (listener, port) =
        bind_listener(run_config.effective_settings.process.web_access, opts.port).await?;

    let token = token_after_bind(&opts, stored);

    let (app, router) = build_web_app(&opts, run_config, host, port, token);

    let shared = app.shared.clone();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

    announce_start(&shared);

    let task = crate::node::output::spawn(accept_loop(
        listener,
        router,
        shared.clone(),
        stop_rx,
        opts.drain,
    ));

    Ok(WebHandle {
        shared,
        stop: stop_tx,
        task,
    })
}

/// The node's own token is on disk before the port opens: a second start with the same settings, seeing
/// the busy port, must already find the token to recognise the running node. A fixed token waits for the bind.
fn stored_token_before_bind(opts: &WebOptions) -> Result<Option<String>, WebStartError> {
    if opts.fixed_token.is_some() {
        return Ok(None);
    }
    match token::resolve_stored_token(opts.token_dir.as_deref()) {
        Ok(token) => Ok(Some(token)),
        Err(reason) => {
            let err = WebStartError::Random(reason);
            crate::node::output::say("web", err.to_string());
            Err(err)
        }
    }
}

/// The token for the running page: the one resolved before the bind, or the fixed one saved now that the
/// port is ours (a failed second start must not overwrite the token of the running node).
fn token_after_bind(opts: &WebOptions, stored: Option<String>) -> String {
    let fixed = opts
        .fixed_token
        .clone()
        .map(|t| token::persist_fixed_token(opts.token_dir.as_deref(), t));
    stored.or(fixed).unwrap_or_default()
}

/// Builds the shared state, the app and the router for the page listening on `port`.
fn build_web_app(
    opts: &WebOptions,
    run_config: &crate::config::config_file::RunConfig,
    host: Arc<dyn HostControl>,
    port: u16,
    token: String,
) -> (Arc<App>, axum::Router) {
    let auth = AuthState::new(
        cookie::session_cookie_name(&run_config.secret.peer_id()),
        opts.token_dir.clone(),
        run_config.effective_settings.process.web_access,
    );
    auth.set_ifaces(run_config.effective_settings.process.web_ifaces.clone());
    let shared = Arc::new(WebShared::new(port, token, auth));
    let app = Arc::new(App {
        shared,
        host,
        token_dir: opts.token_dir.clone(),
        stopping: std::sync::atomic::AtomicBool::new(false),
        pending_invites: Default::default(),
    });
    let router = super::routes::router(app.clone());
    (app, router)
}

/// Listens on `port` at the address given by the access mode and re-reads the actual port (random when
/// `port == 0`).
async fn bind_listener(
    mode: ListenAccess,
    port: u16,
) -> Result<(tokio::net::TcpListener, u16), WebStartError> {
    let ip = bind_ip(mode);
    let bind_err = |e: std::io::Error| WebStartError::Bind {
        port,
        reason: format!("on {ip}: {e}"),
        kind: e.kind(),
    };
    let listener = tokio::net::TcpListener::bind((ip, port))
        .await
        .map_err(bind_err)?;
    let bound_port = listener.local_addr().map_err(bind_err)?.port();
    Ok((listener, bound_port))
}

/// The log line about the page being up: sign-in addresses. The permanent token doesn't get into it.
fn announce_start(shared: &WebShared) {
    let urls = shared.issue_entry_urls();
    let text = announce_text(&urls, shared.auth.password_set());
    crate::node::output::say("web", text);
}

/// The announce text: no password — a link with a one-time token, with a password — sign-in by password.
fn announce_text(urls: &[String], password_set: bool) -> String {
    let addrs = urls.join(" or ");
    if password_set {
        return format!("{NET_NAME} page: {addrs} — password sign-in");
    }
    format!("{NET_NAME} page: {addrs} — sign-in link, valid for 10 minutes, single use")
}

/// Connection addresses; `None` — the peer is not admitted by the current mode or there is no local address:
/// the socket is closed without reading and without a log line.
fn conn_info(
    stream: &tokio::net::TcpStream,
    client_addr: std::net::SocketAddr,
    shared: &WebShared,
) -> Option<ConnInfo> {
    let local = stream.local_addr().ok()?;
    let conn = ConnInfo { client_addr, local };
    let allowed = is_client_allowed(shared.auth.access(), client_addr.ip());
    let iface_ok = allowed && shared.auth.iface_verdict(&current_view(), &conn).allows();
    iface_ok.then_some(conn)
}

/// The connection accept loop: three `select!` branches (stop signal, new connection, collecting
/// finished tasks), then draining started connections with the `drain` deadline and `abort_all()` when
/// it is exceeded. The hand-written accept loop + `JoinSet`/`abort_all` is intentional, not the ready-made
/// graceful shutdown mechanism from `hyper_util::server::graceful` (its combination with a hand-written accept loop
/// was not verified by an axum probe).
async fn accept_loop(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shared: Arc<WebShared>,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
    drain: std::time::Duration,
) {
    let mut conns: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            biased;

            changed = stop_rx.changed() => {
                match changed {
                    Err(_) => break,
                    Ok(()) => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }

            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, peer)) => {
                        if let Some(conn) = conn_info(&stream, peer, &shared) {
                            crate::node::output::spawn_in(&mut conns, super::http::serve_connection(stream, conn, router.clone()));
                        }
                    }
                    Err(e) => {
                        crate::node::output::say("web", format!("page: connection not accepted ({e})"));
                    }
                }
            }

            Some(_) = conns.join_next(), if !conns.is_empty() => {}
        }
    }

    drop(listener);

    let drained =
        tokio::time::timeout(drain, async { while conns.join_next().await.is_some() {} }).await;

    if drained.is_err() {
        let n = conns.len();
        conns.abort_all();
        while conns.join_next().await.is_some() {}
        crate::node::output::say("web", format!("page: {n} connections aborted on stop"));
    }

    crate::node::output::say("web", "page stopped");
}

/// Margin on top of `deadline` for the shutdown tail: covers a late wake-up of `accept_loop`
/// (a worker thread busy with another task) and the tail after draining — `abort_all`, `join_next`, two
/// `output::say` calls. The connection drain itself fits in `deadline` if `deadline ≥ drain`. An implementation detail
/// of `shutdown`, not a startup setting.
/// The value is fixed in code.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

impl WebHandle {
    /// Asks the accept loop to stop and waits for it to finish for no longer than `deadline` plus a small
    /// margin for the shutdown tail (`SHUTDOWN_GRACE`) — it covers only a late wake-up
    /// of the accept loop and the tail after draining (`abort_all`, `join_next`, two `output::say` calls); the connection
    /// drain itself fits in `deadline`. `true` — the task finished in time (successfully or with
    /// a panic), `false` — it didn't make it. To get `true` with hanging connections, `deadline`
    /// must be no less than the `drain` the listener was started with. There is no separate `Drop`: a dropped
    /// `WebHandle` drops `stop`, `changed()` in the loop returns `Err`, and shutdown follows the same path.
    pub async fn shutdown(self, deadline: std::time::Duration) -> bool {
        let _ = self.stop.send_replace(true);
        tokio::time::timeout(deadline.saturating_add(SHUTDOWN_GRACE), self.task)
            .await
            .is_ok()
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
