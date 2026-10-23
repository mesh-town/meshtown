//! Member-list API: DTOs of the reply and the handlers of `/api/members*`.

pub(crate) mod dto;
mod routes;

pub(crate) use routes::{members_value, register_members};
