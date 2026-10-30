//! Command line error texts in English and the handling of a refused command line.
//!
//! - `classify` - why parsing refused, and the exit of the entry point on a refusal.
//! - `text` - the texts: the error report, an unknown command, a wrong option or value.

mod classify;
mod text;

pub(crate) use classify::parse_or_exit;
#[cfg(test)]
pub(crate) use text::bad_value_text;
pub(crate) use text::error_report;
