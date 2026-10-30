//! Command line error texts in English and the handling of a refused command line.
//!
//! - `classify` - why parsing refused, and the exit of the entry point on a refusal.
//! - `text` - the texts: an unknown command, a wrong option or value, a replaced name, any other refusal.

mod classify;
mod text;

pub(crate) use classify::parse_or_exit;
#[cfg(test)]
pub(crate) use text::bad_value_text;
#[cfg(test)]
pub(crate) use classify::classify as classify_for_tests;
#[cfg(test)]
pub(crate) use classify::ParseFail as ParseFailForTests;
