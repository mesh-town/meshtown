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
            assert!(
                !seen.contains(&f.section()),
                "section {} is split",
                f.section()
            );
            seen.push(f.section());
        }
    }
    assert_eq!(seen.len(), 14);
}

#[test]
fn secret_only_network_key() {
    // given: rows with the secret flag
    let secret: Vec<&str> = FIELDS
        .iter()
        .filter(|f| f.redacted)
        .map(|f| f.field_path)
        .collect();
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
    let full = crate::config::config_file::test_support::all_changed();
    let mut got = crate::config::config_file::Settings::default();
    for f in FIELDS {
        f.value.copy_value_between(&full, &mut got);
    }
    assert_eq!(got, full, "a Settings field has no row in the table");
}

#[test]
fn all_changed_differs_in_every_row() {
    // given: defaults and `all_changed()`; when `render` of every stored row; then the values differ
    let (d, full) = (
        crate::config::config_file::Settings::default(),
        crate::config::config_file::test_support::all_changed(),
    );
    for f in FIELDS
        .iter()
        .filter(|f| f.value.is_stored_in_settings() && f.field_path != "net.transports")
    {
        assert_ne!(
            f.value.render_for_file(&d),
            f.value.render_for_file(&full),
            "{}: the output does not tell them apart",
            f.field_path
        );
    }
}

#[test]
fn render_read_roundtrip() {
    // given: `all_changed()` with flags turned off (parsing rejects `true`)
    let mut s = crate::config::config_file::test_support::all_changed();
    s.process.autostart_enabled = false;
    s.process.full_tunnel_enabled = false;
    // when: render and parse
    let secret = crate::transport::net::NodeSecret::from_bytes(&[7; 32]);
    let text = crate::config::config_file::render_config_file(1, 0, &secret, &s);
    let p = crate::config::config_file::parse_text(&text).expect("parsing");
    // then: the same settings and zero errors
    assert_eq!(p.field_errors, vec![]);
    assert_eq!(p.settings, s);
}

#[test]
fn debug_lists_every_stored_row() {
    // given: settings with their own network key
    let mut s = crate::config::config_file::test_support::all_changed();
    s.net.network_key = Some("label--secret-probe-x".into());
    // when: Debug; then every stored path is named, the key value is not
    let text = format!("{s:?}");
    for f in FIELDS.iter().filter(|f| f.value.is_stored_in_settings()) {
        assert!(
            text.contains(&format!("{}:", f.field_path)),
            "missing {}",
            f.field_path
        );
    }
    assert!(
        !text.contains("secret"),
        "the key leaked into Debug: {text}"
    );
}

mod proxy_net_row {
    use super::super::super::row::{Block, Mode};
    use super::FIELDS;

    fn position_of(path: &str) -> usize {
        FIELDS
            .iter()
            .position(|f| f.field_path == path)
            .unwrap_or_else(|| panic!("no row {path}"))
    }

    #[test]
    fn is_a_live_process_row() {
        // given: the `proxy.net` row
        let row = &FIELDS[position_of("proxy.net")];
        // then: it applies without a restart and belongs to the process block
        assert_eq!(row.mode, Mode::Live);
        assert_eq!(row.block(), Block::Process);
        assert_eq!((row.section(), row.key()), ("proxy", "net"));
    }

    #[test]
    fn sits_right_after_socks_ifaces() {
        // given: the row order is the file output order
        // then: `proxy.net` follows `socks.ifaces` and precedes `checks.enabled`
        assert_eq!(position_of("proxy.net"), position_of("socks.ifaces") + 1);
        assert_eq!(position_of("checks.enabled"), position_of("proxy.net") + 1);
    }

    #[test]
    fn adds_one_stored_row_to_the_six_computed_ones() {
        // given: the whole table
        // when: rows stored in `Settings` are counted (computed rows are not)
        let stored = FIELDS
            .iter()
            .filter(|f| f.value.is_stored_in_settings())
            .count();
        // then: 30 stored paths, 6 computed, 36 rows in all
        assert_eq!(stored, 30, "stored rows");
        assert_eq!(FIELDS.len() - stored, 6, "computed rows");
    }
}

mod blocks {
    use super::super::super::row::Block;
    use super::super::super::{rows_of, section_names_of};
    use super::FIELDS;
    use crate::config::config_file::test_support::all_changed;
    use crate::config::config_file::{NetSettings, ProcessSettings, Settings};

    /// Copies the values of every row of `block` from `all_changed()` into default settings.
    fn copy_block_into_defaults(block: Block) -> Settings {
        let full = all_changed();
        let mut got = Settings::default();
        for row in rows_of(block) {
            row.value.copy_value_between(&full, &mut got);
        }
        got
    }

    mod net_rows {
        use super::*;

        #[test]
        fn cover_exactly_the_net_settings() {
            // given: every field differs from the default
            let full = all_changed();
            // when: all `Net` rows are copied into default settings
            let got = copy_block_into_defaults(Block::Net);
            // then: the net block is complete and the process block is untouched
            assert_eq!(
                got,
                Settings {
                    process: ProcessSettings::default(),
                    net: full.net,
                },
                "the `Net` rows do not cover `NetSettings` exactly"
            );
        }

        #[test]
        fn have_no_process_section() {
            // given: the sections of the `Net` block
            let sections = section_names_of(Block::Net);
            // then: none of the process-only sections is among them
            for process_only in ["socks", "web", "log", "autostart", "full_tunnel"] {
                assert!(
                    !sections.contains(&process_only),
                    "section {process_only} leaked into the net block: {sections:?}"
                );
            }
        }

        #[test]
        fn node_section_holds_only_the_name() {
            // given: the `Net` rows of the `node` section
            let keys: Vec<&str> = rows_of(Block::Net)
                .filter(|f| f.section() == "node")
                .map(|f| f.key())
                .collect();
            // then: only `name`
            assert_eq!(keys, vec!["name"]);
        }
    }

    mod process_rows {
        use super::*;

        #[test]
        fn cover_exactly_the_process_settings() {
            // given: every field differs from the default
            let full = all_changed();
            // when: all `Process` rows are copied into default settings
            let got = copy_block_into_defaults(Block::Process);
            // then: the process block is complete and the net block is untouched
            assert_eq!(
                got,
                Settings {
                    process: full.process,
                    net: NetSettings::default(),
                },
                "the `Process` rows do not cover `ProcessSettings` exactly"
            );
        }

        #[test]
        fn memory_limits_belong_to_the_process() {
            // given: the two memory rows (they use different row kinds)
            for path in ["node.memory_warn_mib", "node.memory_exit_mib"] {
                // then: both are process rows
                let row = FIELDS.iter().find(|f| f.field_path == path).expect(path);
                assert_eq!(row.block(), Block::Process, "{path}");
            }
        }
    }

    mod computed_rows {
        use super::*;

        #[test]
        fn follow_the_block_of_their_section() {
            // given: one computed row per block
            let cases = [
                ("network.label", Block::Net),
                ("network.masked", Block::Net),
                ("autostart.available", Block::Process),
                ("full_tunnel.reason", Block::Process),
            ];
            for (path, want) in cases {
                // then: the row names its block
                let row = FIELDS.iter().find(|f| f.field_path == path).expect(path);
                assert_eq!(row.block(), want, "{path}");
            }
        }
    }

    #[test]
    fn the_blocks_split_the_table_without_remainder() {
        // given: the whole table
        // when: the rows of both blocks are counted
        let total = rows_of(Block::Net).count() + rows_of(Block::Process).count();
        // then: every row belongs to exactly one block
        assert_eq!(total, FIELDS.len());
    }
}
