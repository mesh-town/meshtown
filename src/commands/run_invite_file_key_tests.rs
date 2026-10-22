use super::*;

mod of_the_same_network {
    use super::*;

    #[test]
    fn keeps_the_key_and_does_not_spend_the_invite() {
        // given
        let opts = Opts {
            file_key: Some(net_a().encode()),
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        assert!(matches!(out, Outcome::KeepKey), "{out:?}");
    }

    #[test]
    fn keeps_the_key_written_with_a_label_even_with_save_config() {
        // given
        let opts = Opts {
            file_key: Some(key_with_label(Some("our-game"), &net_a())),
            save_config: true,
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        assert!(matches!(out, Outcome::KeepKey), "{out:?}");
    }
}

mod of_another_network {
    use super::*;

    mod without_save_config {
        use super::*;

        #[test]
        fn refuses_naming_the_flag_and_not_spending_the_invite() {
            // given
            let opts = Opts {
                file_key: Some(net_b().encode()),
                ..Opts::default()
            };
            // when
            let text = plan(opts).refusal();
            // then
            assert!(text.contains("--save-config"), "{text}");
            assert!(text.contains("was not spent"), "{text}");
        }
    }

    mod with_save_config {
        use super::*;

        #[test]
        fn redeems_the_invite() {
            // given
            let opts = Opts {
                file_key: Some(net_b().encode()),
                save_config: true,
                ..Opts::default()
            };
            // when
            let out = plan(opts);
            // then
            then_redeems_the_same_invite(out);
        }
    }
}
