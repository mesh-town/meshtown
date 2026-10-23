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
    fn keeps_the_key_written_with_a_label() {
        // given
        let opts = Opts {
            file_key: Some(key_with_label(Some("our-game"), &net_a())),
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

    #[test]
    fn adds_a_network_slot() {
        // given
        let opts = Opts {
            file_key: Some(net_b().encode()),
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        then_adds_a_network_with_the_same_invite(out);
    }

    #[test]
    fn adds_a_network_slot_beside_other_slots() {
        // given: the plan has no input that could replace the file's key
        let opts = Opts {
            file_key: Some(net_b().encode()),
            slot_keys: vec![Some(net_c().encode())],
            ..Opts::default()
        };
        // when
        let out = plan(opts);
        // then
        then_adds_a_network_with_the_same_invite(out);
    }
}
