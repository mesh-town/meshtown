use super::factory_for;

#[test]
fn lan_discovery_is_off_with_a_given_bootstrap() {
    let factory = factory_for(Some(vec!["127.0.0.1:1".into()]), None);

    assert!(!factory.lan_discovery);
}

#[test]
fn lan_discovery_is_off_with_the_environment_defaults() {
    let factory = factory_for(None, None);

    assert!(!factory.lan_discovery);
}
