//! QR code of an invite string, drawn by the node as an SVG document.

use qrcode::render::svg::Color;
use qrcode::{EcLevel, QrCode, Version};
use serde_json::{json, Value};

/// Largest QR version that stays easy to scan from a screen (117x117 modules).
pub(crate) const QR_MAX_VERSION: i16 = 25;
/// Error correction level: the code is shown on a screen, so damage is unlikely.
pub(crate) const QR_EC_LEVEL: EcLevel = EcLevel::L;

/// Smallest code that holds `text`, or `None` if it needs a version above `QR_MAX_VERSION`.
pub(crate) fn invite_qr_code(text: &str) -> Option<QrCode> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), QR_EC_LEVEL).ok()?;
    let fits = code.version().width() <= Version::Normal(QR_MAX_VERSION).width();
    fits.then_some(code)
}

/// SVG of the code: one unit per module, black on white, with the standard quiet zone.
pub(crate) fn invite_qr_svg(text: &str) -> Option<String> {
    let code = invite_qr_code(text)?;
    Some(code.render::<Color>().module_dimensions(1, 1).build())
}

/// Adds the `qr` field (an SVG string or `null`) to an invite response body.
pub(crate) fn with_qr(mut body: Value) -> Value {
    let svg = body["invite"].as_str().and_then(invite_qr_svg);
    body["qr"] = json!(svg);
    body
}

#[cfg(test)]
#[path = "invite_qr_tests.rs"]
mod tests;
