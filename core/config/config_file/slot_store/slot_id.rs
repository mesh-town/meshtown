//! `NetSlotId`: the name of a network slot — `main` for the primary one, six base32 characters for the others.

use std::fmt;

const MAIN: &str = "main";
const SUFFIX_LEN: usize = 6;

/// Identifier of a network slot. Only valid names can be built, so it is safe to use as a directory name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetSlotId(String);

impl NetSlotId {
    /// The primary slot.
    pub fn main() -> Self {
        NetSlotId(MAIN.to_string())
    }

    /// `main` or exactly six characters of `[a-z2-7]`; anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let valid = text == MAIN || (text.len() == SUFFIX_LEN && text.bytes().all(is_base32));
        valid.then(|| NetSlotId(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_base32(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'2'..=b'7')
}

impl fmt::Display for NetSlotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "slot_id_tests.rs"]
mod tests;
