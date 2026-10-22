use super::has_field;

fn file(socks: &str) -> String {
    format!(r#"{{"rev":1,"config":{{"version":1,"socks":{socks}}}}}"#)
}

#[test]
fn empty_list_and_null_count_as_present() {
    assert!(has_field(&file(r#"{"ifaces":[]}"#), "socks.ifaces"));
    assert!(has_field(&file(r#"{"ifaces":null}"#), "socks.ifaces"));
}

#[test]
fn missing_key_or_section_is_absent() {
    assert!(!has_field(&file(r#"{"port":1080}"#), "socks.ifaces"));
    assert!(!has_field(&file(r#"{"ifaces":[]}"#), "web.ifaces"));
}

#[test]
fn garbage_is_absent() {
    assert!(!has_field("not json", "socks.ifaces"));
    assert!(!has_field("[]", "socks.ifaces"));
    assert!(!has_field(&file("{}"), "ifaces"));
}

#[test]
fn key_outside_config_section_is_absent() {
    let text = r#"{"rev":1,"socks":{"ifaces":[]},"config":{"version":1}}"#;
    assert!(!has_field(text, "socks.ifaces"));
}
