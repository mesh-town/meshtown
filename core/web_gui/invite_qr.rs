//! QR code of an invite string, drawn by the node as an SVG document.

use qrcode::render::svg::Color;
use qrcode::{EcLevel, QrCode, Version};
use serde_json::{json, Value};

/// Largest QR version that stays easy to scan from a screen (117x117 modules).
pub(crate) const QR_MAX_VERSION: i16 = 25;
/// Error correction level: the code is shown on a screen, so damage is unlikely.
pub(crate) const QR_EC_LEVEL: EcLevel = EcLevel::L;

/// Smallest code that holds `text`, or `None` if it needs a version above `QR_MAX_VERSION`.
///
/// The code is built from the upper-case text: base32 letters, digits and `-` all belong to the QR
/// alphanumeric mode (5.5 bits per character instead of 8), so the same invite fits a smaller version.
/// Invite parsing ignores case, so a scanner that returns the upper-case text still gives a valid invite.
pub(crate) fn invite_qr_code(text: &str) -> Option<QrCode> {
    let upper = text.to_ascii_uppercase();
    let code = QrCode::with_error_correction_level(upper.as_bytes(), QR_EC_LEVEL).ok()?;
    let fits = code.version().width() <= Version::Normal(QR_MAX_VERSION).width();
    fits.then_some(code)
}

/// SVG of the code: one unit per module, black on white, with the standard quiet zone.
pub(crate) fn invite_qr_svg(text: &str) -> Option<String> {
    let code = invite_qr_code(text)?;
    Some(code.render::<Color>().module_dimensions(1, 1).build())
}

/// Adds the `qr` field (an SVG string or `null`) to an invite response body; the picture is drawn from
/// `qr_text` (the shorter form of the invite), and only when the body has an invite.
pub(crate) fn with_qr(mut body: Value, qr_text: &str) -> Value {
    let svg = body["invite"].as_str().and_then(|_| invite_qr_svg(qr_text));
    body["qr"] = json!(svg);
    body
}

#[cfg(test)]
#[path = "invite_qr_tests.rs"]
mod tests;
