//! The structure of the `FIELDS` table.

use super::super::row::Mode;
use super::FIELDS;
use std::collections::BTreeSet;

#[test]
fn paths_unique() {
    // given: the whole table
    let set: BTreeSet<&str> = FIELDS.iter().map(|f| f.field_path).collect();
    // then: 36 rows, no repeats
    assert_eq!(FIELDS.len(), 36);
    assert_eq!(set.len(), FIELDS.len(), "a path is repeated");
}

#[test]
fn sections_contiguous() {
    // given: sections in row order
    let mut seen: Vec<&str> = Vec::new();
    for f in FIELDS {
        // when: the section changed
        if seen.last() != Some(&f.section()) {
            // then: it was not visited before
            assert!(!seen.contains(&f.section()), "section {} is split", f.section());
            seen.push(f.section());
        }
    }
    assert_eq!(seen.len(), 14);
}

#[test]
fn secret_only_network_key() {
    // given: rows with the secret flag
    let secret: Vec<&str> = FIELDS.iter().filter(|f| f.redacted).map(|f| f.field_path).collect();
    // then: there is exactly one
    assert_eq!(secret, vec!["network.key"]);
}

#[test]
fn computed_rows_read_only() {
    // given: 6 paths that nothing in `Settings` answers to
    let computed = [
        "network.label",
        "network.masked",
        "autostart.available",
        "autostart.reason",
        "full_tunnel.available",
        "full_tunnel.reason",
    ];
    for path in computed {
        // then: the row exists and its mode is ReadOnly
        let row = FIELDS.iter().find(|f| f.field_path == path).expect(path);
        assert_eq!(row.mode, Mode::ReadOnly, "{path}");
    }
}

#[test]
fn mode_editable_by_mode() {
    // given: six modes
    let yes = [Mode::Live, Mode::RestartShown, Mode::EditOnly];
    let no = [Mode::ReadOnly, Mode::FileOnly];
    // then: true for exactly four
    assert!(yes.iter().all(|m| m.is_editable()));
    assert!(no.iter().all(|m| !m.is_editable()));
}

#[test]
fn every_settings_field_has_row() {
    // given: defaults and `all_changed()`; when copying all rows; then the result equals `all_changed()`
    let full = crate::config_file::test_support::all_changed();
    let mut got = crate::config_file::Settings::default();
    for f in FIELDS {
        f.value.copy_value_between(&full, &mut got);
    }
    assert_eq!(got, full, "a Settings field has no row in the table");
}

#[test]
fn all_changed_differs_in_every_row() {
    // given: defaults and `all_changed()`; when `render` of every stored row; then the values differ
    let (d, full) = (crate::config_file::Settings::default(), crate::config_file::test_support::all_changed());
    for f in FIELDS.iter().filter(|f| f.value.is_stored_in_settings() && f.field_path != "net.transports") {
        assert_ne!(f.value.render_for_file(&d), f.value.render_for_file(&full), "{}: the output does not tell them apart", f.field_path);
    }
}

#[test]
fn render_read_roundtrip() {
    // given: `all_changed()` with flags turned off (parsing rejects `true`)
    let mut s = crate::config_file::test_support::all_changed();
    s.process.autostart_enabled = false;
    s.process.full_tunnel_enabled = false;
    // when: render and parse
    let secret = crate::net::NodeSecret::from_bytes(&[7; 32]);
    let text = crate::config_file::render_config_file(1, 0, &secret, &s);
    let p = crate::config_file::parse_text(&text).expect("parsing");
    // then: the same settings and zero errors
    assert_eq!(p.field_errors, vec![]);
    assert_eq!(p.settings, s);
}

#[test]
fn debug_lists_every_stored_row() {
    // given: settings with their own network key
    let mut s = crate::config_file::test_support::all_changed();
    s.net.network_key = Some("label--secret-probe-x".into());
    // when: Debug; then every stored path is named, the key value is not
    let text = format!("{s:?}");
    for f in FIELDS.iter().filter(|f| f.value.is_stored_in_settings()) {
        assert!(text.contains(&format!("{}:", f.field_path)), "missing {}", f.field_path);
    }
    assert!(!text.contains("secret"), "the key leaked into Debug: {text}");
}
