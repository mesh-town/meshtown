//! Member-list API: DTOs of the reply and the handlers of `/api/members*`.

pub(crate) mod dto;
mod leave;
mod routes;

pub(crate) use routes::{members_value, register_members, MAX_REASON_CHARS};
