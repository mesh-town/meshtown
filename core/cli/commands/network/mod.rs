//! The `network` command group. For now it holds only the key printing of `keygen`.

mod key_only;

pub(crate) use key_only::keygen;
