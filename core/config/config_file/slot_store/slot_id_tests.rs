use super::NetSlotId;

#[test]
fn accepts_main_and_six_base32_chars() {
    assert_eq!(NetSlotId::parse("main"), Some(NetSlotId::main()));
    assert_eq!(NetSlotId::parse("ab2c3d").unwrap().as_str(), "ab2c3d");
}

#[test]
fn rejects_everything_else() {
    for bad in ["AB2C3D", "ab2c3", "../x", "ab2c3d1", "", "ab1c3d", "Main"] {
        assert_eq!(NetSlotId::parse(bad), None, "{bad}");
    }
}

#[test]
fn displays_as_its_name() {
    assert_eq!(NetSlotId::main().to_string(), "main");
}
