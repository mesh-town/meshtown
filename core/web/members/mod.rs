//! Member-list API: DTOs of the reply and the handlers of `/api/members*`.

pub(crate) mod dto;
mod routes;

pub(crate) use routes::register_members;
