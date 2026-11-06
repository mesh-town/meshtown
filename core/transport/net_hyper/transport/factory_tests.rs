use super::{base_factory, factory_for};

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

#[test]
fn base_factory_limits_the_receive_window() {
    for bootstrap in [Some(vec!["127.0.0.1:1".to_string()]), None] {
        let limits = base_factory(bootstrap).stream_limits;

        assert_eq!(limits.recv_window_max, Some(262144));
        assert_eq!(limits.send_window_max, None);
        assert_eq!(limits.write_high_watermark, None);
    }
}
