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

mod carries_process {
    use super::*;
    use crate::config::config_file::slot_store::ids::new_slot_id_with;
    use crate::config::config_file::slot_store::net_slot_path;
    use std::path::Path;

    #[test]
    fn main_slot_carries_the_process_block() {
        // given: the primary slot
        let id = NetSlotId::main();
        // when / then: it carries the process block
        assert!(id.carries_process());
    }

    #[test]
    fn added_slot_does_not() {
        // given: an id picked for an added slot
        let id = added_id();
        // when / then: it does not carry the process block
        assert!(!id.carries_process(), "{id}");
    }

    #[test]
    fn agrees_with_having_no_file_of_its_own() {
        // given: the primary slot and an added one
        let main = Path::new("settings/config.json");
        for id in [NetSlotId::main(), added_id()] {
            // when: the slot file path is asked for
            let has_no_own_file = net_slot_path(main, &id).is_none();
            // then: only a slot that carries the process has no file of its own
            assert_eq!(has_no_own_file, id.carries_process(), "{id}");
        }
    }

    fn added_id() -> NetSlotId {
        let main = Path::new("settings/config.json");
        new_slot_id_with(main, || "ab2c3d".to_string()).expect("a free id")
    }
}
