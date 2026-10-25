//! Turns a member name into a DNS-safe address label.

use icu_normalizer::DecomposingNormalizerBorrowed;

/// Longest label produced by [`host_label`].
const MAX_LABEL: usize = 50;
/// Label used when a name has no usable letters or digits.
const FALLBACK_LABEL: &str = "node";
/// Latin letters that do not decompose into a base letter and marks.
const LETTERS: [(char, &str); 8] = [
    ('ß', "ss"),
    ('æ', "ae"),
    ('œ', "oe"),
    ('ø', "o"),
    ('ł', "l"),
    ('đ', "d"),
    ('þ', "th"),
    ('ı', "i"),
];

/// Address label for a member name: lower-case ASCII letters and digits joined by single
/// dashes, at most 50 characters, never empty. Accents are dropped (`Café` becomes `cafe`);
/// scripts without a Latin base letter turn into separators.
pub fn host_label(name: &str) -> String {
    let folded: String = decomposed(name).chars().map(piece).collect();
    let joined = folded
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    finish(&joined)
}

fn decomposed(name: &str) -> String {
    let lower = name.to_lowercase();
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(&lower)
        .into_owned()
}

/// What one decomposed character contributes: itself, a replacement, nothing or a separator.
fn piece(c: char) -> String {
    if c.is_ascii_alphanumeric() {
        return c.to_string();
    }
    if is_combining_mark(c) {
        return String::new();
    }
    let replacement = LETTERS.iter().find(|(letter, _)| *letter == c);
    replacement.map_or_else(|| "-".to_string(), |(_, text)| (*text).to_string())
}

fn is_combining_mark(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
}

fn finish(joined: &str) -> String {
    let cut: String = joined.chars().take(MAX_LABEL).collect();
    let label = cut.trim_end_matches('-');
    if label.is_empty() {
        return FALLBACK_LABEL.to_string();
    }
    label.to_string()
}

#[cfg(test)]
#[path = "host_label_tests.rs"]
mod tests;
