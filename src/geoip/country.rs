//! An ISO 3166-1 alpha-2 country code — two ASCII bytes, uppercased.
//!
//! What it does: parses and stores strictly two Latin characters as a country code; rejects
//! everything else and the code `XX` (that is how Cloudflare writes "unknown").
//! What it doesn't do: doesn't check that the code really exists in the ISO list — only the shape.
//! Links: used by `geoip::sources` (parsing source answers) and `geoip::board`
//! (writing and reading the network's shared document).

#[cfg(test)]
#[path = "country_tests.rs"]
mod tests;

/// A country code — two ASCII bytes in capital letters, for example `RU`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Country([u8; 2]);

impl Country {
    /// Parses a country code from a string: exactly two Latin letters, `XX` — "no country".
    pub fn parse(s: &str) -> Option<Country> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let upper = uppercase_letters(bytes[0], bytes[1])?;
        if &upper == b"XX" {
            return None;
        }
        Some(Country(upper))
    }

    /// The same for the bytes of a value from the network's shared document — non-UTF8 is also "no country".
    pub fn from_bytes(b: &[u8]) -> Option<Country> {
        Country::parse(std::str::from_utf8(b).ok()?)
    }

    /// The code in capital letters, for example `"RU"`.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("Country holds only ASCII letters")
    }
}

/// Both letters are Latin alphabet, uppercased; otherwise the code is invalid.
fn uppercase_letters(a: u8, b: u8) -> Option<[u8; 2]> {
    if !a.is_ascii_alphabetic() || !b.is_ascii_alphabetic() {
        return None;
    }
    Some([a.to_ascii_uppercase(), b.to_ascii_uppercase()])
}
