use super::*;

fn overrides_with_key(key: &str) -> CliOverrides {
    CliOverrides {
        network_key: Some(key.to_string()),
        ..CliOverrides::default()
    }
}

#[test]
fn an_unreadable_key_is_not_given_to_the_main_network() {
    let main = overrides_for_main(&overrides_with_key("garbage"), &KeyNet::Unreadable);
    assert_eq!(main.network_key, None);
}

#[test]
fn the_main_network_keeps_the_cli_key() {
    let main = overrides_for_main(&overrides_with_key("garbage"), &KeyNet::Main);
    assert_eq!(main.network_key.as_deref(), Some("garbage"));
}
