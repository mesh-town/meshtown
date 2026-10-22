//! A custom HTTP/1.0 client for the node page on `127.0.0.1`.
//!
//! Files: `request.rs` — the request bytes (`request_bytes_without_body`); `transport.rs` — the TCP exchange with timeouts
//! (`exchange`/`send_request_read_response`); `response.rs` — pure parsing of the raw response bytes
//! (`parse_response`); `classify.rs` — classification into `Reply`/`ApiError` (`WEB-API.md`);
//! `types.rs` — the `Reply`/`ApiError` types themselves; `client.rs` — thin wrappers `get_state`/`post_stop`
//! over the whole chain; `fixtures.rs` — the fake node for tests, declared as the module `tests` so that
//! the path `local_api::tests::…` does not change — the `tray_role` module reads it too.
//! What it does not do: retry on 401, re-read the token, or decide the further
//! fate of a `Reply`/`ApiError` — that is the `tray_role` poller, which does not look here yet
//! (it is still a stub): `dead_code` on `get_state`/`post_stop` in a normal build is expected and allowed here
//! until the poller uses them.
//! Links: the page contract is `WEB-API.md`; the node-side checks are `web/guard.rs`,
//! `web/routes.rs`; `fixtures::start_fake` starts a real `meshtown::web::start_web_page` with
//! the fake node `FakeNode`.

mod classify;
mod client;
mod request;
mod response;
mod transport;
mod types;

pub(crate) use client::{get_config, get_invite, get_state, post_stop, put_config};
pub(crate) use types::{ApiError, Reply};

#[cfg(test)]
#[path = "fixtures.rs"]
pub(crate) mod tests;
