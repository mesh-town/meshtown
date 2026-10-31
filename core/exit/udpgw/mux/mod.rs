//! Client side of the UDP gateway: many local gateway connections share a few streams to the exit.
//!
//! Each local connection keeps its own `conn_id` numbering; on the shared stream every binding gets
//! its own id, so two connections may both use id 1. Frames are forwarded as raw bytes, only the two
//! `conn_id` bytes are rewritten.
//!
//! Files: `frame` — raw frame reading and `conn_id` rewriting; `ids` — the pure table that maps local
//! ids to shared ids; `lane` — one shared stream with its writer and reader tasks; `local` — the
//! loop serving one local connection.

mod frame;
mod ids;
mod lane;
mod local;
