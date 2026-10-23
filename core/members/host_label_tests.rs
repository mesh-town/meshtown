use super::*;

mod latin_names {
    use super::*;

    #[test]
    fn are_lowercased_with_words_joined_by_a_dash() {
        assert_eq!(host_label("Home OpenWrt"), "home-openwrt");
    }

    #[test]
    fn keep_digits() {
        assert_eq!(host_label("Router 2"), "router-2");
    }

    #[test]
    fn already_valid_label_is_unchanged() {
        assert_eq!(host_label("box-7"), "box-7");
    }
}

mod accented_letters {
    use super::*;

    #[test]
    fn lose_their_accent() {
        assert_eq!(host_label("Café Router"), "cafe-router");
    }

    #[test]
    fn with_several_accents_in_one_word_are_all_stripped() {
        assert_eq!(host_label("Crème Brûlée"), "creme-brulee");
    }
}

mod letters_without_decomposition {
    use super::*;

    #[test]
    fn sharp_s_and_slashed_o_are_transliterated() {
        assert_eq!(host_label("Straße Øst"), "strasse-ost");
    }

    #[test]
    fn ae_ligature_becomes_ae() {
        assert_eq!(host_label("Æther"), "aether");
    }

    #[test]
    fn oe_ligature_becomes_oe() {
        assert_eq!(host_label("Cœur"), "coeur");
    }

    #[test]
    fn barred_l_becomes_l() {
        assert_eq!(host_label("Łódź"), "lodz");
    }

    #[test]
    fn barred_d_becomes_d() {
        assert_eq!(host_label("Đorđe Box"), "dorde-box");
    }

    #[test]
    fn thorn_becomes_th() {
        assert_eq!(host_label("þing"), "thing");
    }

    #[test]
    fn dotless_i_becomes_i() {
        assert_eq!(host_label("kıtchen"), "kitchen");
    }
}

mod separators {
    use super::*;

    #[test]
    fn runs_collapse_into_one_dash_and_edges_are_trimmed() {
        assert_eq!(host_label("  my__router..2 "), "my-router-2");
    }

    #[test]
    fn dashes_in_a_row_collapse_into_one() {
        assert_eq!(host_label("a---b"), "a-b");
    }

    #[test]
    fn leading_and_trailing_dashes_are_removed() {
        assert_eq!(host_label("-box-"), "box");
    }

    #[test]
    fn punctuation_between_words_acts_as_a_separator() {
        assert_eq!(host_label("Home's (Router)"), "home-s-router");
    }
}

mod names_without_usable_letters {
    use super::*;

    #[test]
    fn non_latin_script_falls_back_to_node() {
        assert_eq!(host_label("\u{53f0}\u{6240}"), "node");
    }

    #[test]
    fn empty_name_falls_back_to_node() {
        assert_eq!(host_label(""), "node");
    }

    #[test]
    fn only_dashes_fall_back_to_node() {
        assert_eq!(host_label("---"), "node");
    }

    #[test]
    fn only_whitespace_falls_back_to_node() {
        assert_eq!(host_label(" \t "), "node");
    }

    #[test]
    fn non_latin_part_is_dropped_next_to_latin_words() {
        assert_eq!(host_label("Kitchen \u{53f0}\u{6240} Box"), "kitchen-box");
    }
}

mod long_names {
    use super::*;

    #[test]
    fn are_cut_to_at_most_fifty_characters() {
        let label = host_label(&"a".repeat(200));
        assert!(label.len() <= 50, "label is {} bytes long", label.len());
    }

    #[test]
    fn cut_inside_a_separator_leaves_no_trailing_dash() {
        // Words of four letters plus a dash put a dash exactly at position 50.
        let label = host_label(&"word ".repeat(40));
        assert!(!label.ends_with('-'), "label is {label:?}");
    }

    #[test]
    fn keep_their_beginning() {
        let label = host_label(&"word ".repeat(40));
        assert!(label.starts_with("word-word-"), "label is {label:?}");
    }

    #[test]
    fn made_of_non_latin_characters_fall_back_to_node() {
        assert_eq!(host_label(&"\u{53f0}".repeat(200)), "node");
    }
}

mod any_result {
    use super::*;

    const SAMPLES: &[&str] = &[
        "Home OpenWrt",
        "Café Router",
        "Straße Øst",
        "  my__router..2 ",
        "\u{53f0}\u{6240}",
        "",
        "---",
        "-",
        "a b c",
        "UPPER_CASE.name",
        "x\u{0}y",
        "emoji \u{1f600} box",
    ];

    #[test]
    fn is_never_empty() {
        for name in SAMPLES {
            assert!(!host_label(name).is_empty(), "empty label for {name:?}");
        }
    }

    #[test]
    fn uses_only_lowercase_ascii_letters_digits_and_dashes() {
        for name in SAMPLES {
            let label = host_label(name);
            let valid = label
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            assert!(valid, "label {label:?} for {name:?} is not a DNS label");
        }
    }

    #[test]
    fn neither_starts_nor_ends_with_a_dash() {
        for name in SAMPLES {
            let label = host_label(name);
            assert!(
                !label.starts_with('-') && !label.ends_with('-'),
                "label {label:?} for {name:?} has a dash at an edge"
            );
        }
    }

    #[test]
    fn is_stable_when_applied_to_its_own_output() {
        for name in SAMPLES {
            let once = host_label(name);
            assert_eq!(host_label(&once), once, "not idempotent for {name:?}");
        }
    }
}
