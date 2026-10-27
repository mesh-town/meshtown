use super::*;

mod default {
    use super::*;

    #[test]
    fn is_network() {
        assert_eq!(RelayMode::default(), RelayMode::Network);
    }
}

mod parse {
    use super::*;

    #[test]
    fn knows_off_and_network() {
        assert_eq!(RelayMode::parse("off"), Some(RelayMode::Off));
        assert_eq!(RelayMode::parse("network"), Some(RelayMode::Network));
    }

    #[test]
    fn rejects_other_strings() {
        for s in ["any", "", "Off", "NETWORK", " off", "on"] {
            assert_eq!(RelayMode::parse(s), None, "{s:?} was accepted");
        }
    }

    #[test]
    fn is_the_inverse_of_as_str() {
        for mode in [RelayMode::Off, RelayMode::Network] {
            assert_eq!(RelayMode::parse(mode.as_str()), Some(mode));
        }
    }
}
