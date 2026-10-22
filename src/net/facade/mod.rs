//! Network facade: `NetStack` and the `NetStackBuilder` builder (there is no logic here).
//! `services` — handlers and acceptor, `builder` — building, `stack` — operation and closing.

mod builder;
mod services;
mod stack;
#[cfg(test)]
pub(crate) mod test_support;

pub use builder::NetStackBuilder;
pub use services::ServiceHandler;
pub use stack::{NetSnapshot, NetStack, WeakStack};
