//! Tests of the invite QR code: which texts get a code, what the SVG draws, and the `qr` body field.

use std::collections::BTreeSet;

use qrcode::{Color, QrCode, Version};
use serde_json::json;

use super::*;

/// Size of the quiet zone, in modules, on every side of the code.
const QUIET_ZONE: usize = 4;
/// The longest text that fits version 25 at the lowest error correction level in byte mode.
const LIMIT: usize = 1273;

#[test]
fn short_invite_gets_the_smallest_code() {
    // given: an invite of one member
    let text = invite_like(159);
    // when
    let code = invite_qr_code(&text).expect("a short invite fits");
    let svg = invite_qr_svg(&text).expect("a short invite has a picture");
    // then: a small version, the lowest correction level, the picture is the modules plus the quiet zone
    let smallest = QrCode::with_error_correction_level(&text, QR_EC_LEVEL).unwrap();
    assert_eq!(
        code.version(),
        smallest.version(),
        "not the smallest version"
    );
    assert!(
        matches!(code.version(), Version::Normal(v) if v <= 10),
        "{:?}",
        code.version()
    );
    assert_eq!(code.error_correction_level(), QR_EC_LEVEL);
    let side = code.width() + 2 * QUIET_ZONE;
    assert_eq!(svg_side(&svg), side);
    assert!(
        svg.contains(&format!("viewBox=\"0 0 {side} {side}\"")),
        "{svg}"
    );
}

#[test]
fn svg_draws_exactly_the_dark_modules_of_the_text() {
    // given: a typical invite and one with a mixed alphabet
    for text in [invite_like(754), varied_invite(754)] {
        // when
        let svg = invite_qr_svg(&text).expect("fits");
        let code = QrCode::with_error_correction_level(&text, QR_EC_LEVEL).unwrap();
        // then: the picture has the dark modules of the code built again from the same text, and no others
        let drawn = dark_in_svg(&svg);
        assert!(!drawn.is_empty(), "nothing is drawn");
        assert_eq!(drawn, dark_in_code(&code));
        assert_eq!(
            svg.matches('M').count(),
            drawn.len(),
            "a module is drawn twice"
        );
    }
}

#[test]
fn invite_at_the_limit_gets_a_version_25_code() {
    // given: the longest text that fits
    let text = invite_like(LIMIT);
    // when
    let code = invite_qr_code(&text);
    // then
    let code = code.expect("the longest fitting invite has a code");
    assert_eq!(code.version(), Version::Normal(QR_MAX_VERSION));
    assert!(invite_qr_svg(&text).is_some());
}

#[test]
fn one_char_over_the_limit_gets_no_code() {
    // given: one character more than version 25 holds
    let text = invite_like(LIMIT + 1);
    // when / then: neither the code nor the picture, although a bigger version could hold it
    assert!(invite_qr_code(&text).is_none());
    assert!(invite_qr_svg(&text).is_none());
}

#[test]
fn text_too_long_for_any_version_gets_no_code() {
    // given: more than any version holds
    let text = invite_like(4000);
    // when / then
    assert!(invite_qr_code(&text).is_none());
    assert!(invite_qr_svg(&text).is_none());
}

#[test]
fn qr_is_added_next_to_the_other_fields() {
    // given: a usual response body
    let text = invite_like(159);
    let body = json!({"invite": text, "holders": 2, "reach": "spread"});
    // when
    let with = with_qr(body);
    // then: the picture of the same text is added and nothing else changes
    assert_eq!(with["qr"], json!(invite_qr_svg(&text).unwrap()));
    assert_eq!(
        (&with["invite"], &with["holders"], &with["reach"]),
        (&json!(text), &json!(2), &json!("spread"))
    );
    assert_eq!(with.as_object().unwrap().len(), 4);
}

#[test]
fn long_invite_body_gets_null_qr() {
    // given: a body whose invite does not fit
    let text = invite_like(LIMIT + 1);
    let body = json!({"invite": text, "holders": 1, "reach": "issuer_and_one"});
    // when
    let with = with_qr(body);
    // then: the field is there and null, the rest is as it was
    assert!(with.as_object().unwrap().contains_key("qr"), "no qr field");
    assert!(with["qr"].is_null());
    assert_eq!(
        (&with["invite"], &with["holders"]),
        (&json!(text), &json!(1))
    );
}

#[test]
fn body_without_invite_gets_null_qr() {
    // given: bodies with no invite, a number, and a null in its place
    let bodies = [
        json!({"holders": 1}),
        json!({"invite": 42}),
        json!({"invite": null}),
    ];
    for body in bodies {
        // when
        let with = with_qr(body.clone());
        // then: null qr, the other fields are kept
        assert!(with.as_object().unwrap().contains_key("qr"), "{body}");
        assert!(with["qr"].is_null(), "{body}");
        for (key, value) in body.as_object().unwrap() {
            assert_eq!(&with[key], value, "{key}");
        }
    }
}

#[test]
fn helper_reads_back_a_hand_written_path() {
    // given: a path of two modules inside the quiet zone offset
    let svg = r##"<svg width="9"><path fill="#000" d="M4 4h1v1H4V4M6 5h1v1H6V5"/></svg>"##;
    // when / then
    assert_eq!(dark_in_svg(svg), BTreeSet::from([(0, 0), (2, 1)]));
}

/// An invite-shaped text of exactly `len` characters: the prefix and lowercase letters.
fn invite_like(len: usize) -> String {
    let prefix = "mesh-invite2-";
    format!("{prefix}{}", "a".repeat(len - prefix.len()))
}

/// Like `invite_like`, but with the base32 alphabet repeated, so the code has no regular pattern.
fn varied_invite(len: usize) -> String {
    let prefix = "mesh-invite2-";
    let tail: String = "abcdefghijklmnopqrstuvwxyz234567"
        .chars()
        .cycle()
        .take(len - prefix.len())
        .collect();
    format!("{prefix}{tail}")
}

/// The `width` of the root element: modules plus the quiet zone on both sides.
fn svg_side(svg: &str) -> usize {
    let rest = svg.split("width=\"").nth(1).expect("no width attribute");
    rest.split('"')
        .next()
        .unwrap()
        .parse()
        .expect("width is a number")
}

/// Coordinates of the dark modules drawn by the path, in module units without the quiet zone.
/// Every dark module must be one unit square written as `M{x} {y}h1v1H{x}V{y}`.
fn dark_in_svg(svg: &str) -> BTreeSet<(usize, usize)> {
    let path = svg
        .split(" d=\"")
        .nth(1)
        .expect("no path")
        .split('"')
        .next()
        .unwrap();
    path.split('M')
        .filter(|s| !s.is_empty())
        .map(module_in_path)
        .collect()
}

fn module_in_path(segment: &str) -> (usize, usize) {
    let (x, rest) = segment.split_once(' ').expect("x and y");
    let (y, tail) = rest.split_once('h').expect("a horizontal side");
    assert_eq!(tail, format!("1v1H{x}V{y}"), "not a unit square: {segment}");
    let inside = |v: &str| v.parse::<usize>().unwrap().checked_sub(QUIET_ZONE);
    (
        inside(x).expect("a module in the quiet zone"),
        inside(y).expect("a module in the quiet zone"),
    )
}

/// Coordinates of the dark modules of the code, from its colors in row order.
fn dark_in_code(code: &QrCode) -> BTreeSet<(usize, usize)> {
    let width = code.width();
    let colors = code.to_colors();
    let at = |i: usize| (i % width, i / width);
    let dark = colors
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == Color::Dark);
    dark.map(|(i, _)| at(i)).collect()
}
