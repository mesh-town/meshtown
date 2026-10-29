use super::*;
use windows_sys::Win32::System::Registry::{RegDeleteKeyW, REG_BINARY};

fn delete_tree(key: &str) {
    let key = wide(key);
    // SAFETY: the key is zero-terminated and outlives the call.
    unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
}

#[test]
fn sz_round_trips_with_closing_zero() {
    let bytes = encode_sz("\"C:\\a b\\x.exe\" tray");
    assert_eq!(bytes.len() % 2, 0);
    assert_eq!(&bytes[bytes.len() - 2..], &[0, 0]);
    assert_eq!(decode_sz(&bytes), "\"C:\\a b\\x.exe\" tray");
}

#[test]
fn enable_then_disable_in_test_subkey() {
    let root = format!(r"Software\meshtown-test-{}", std::process::id());
    let entry = RegistryEntry {
        run_key: format!(r"{root}\Run"),
        approved_key: format!(r"{root}\Approved"),
        name: "Mesh Town".to_string(),
    };
    assert_eq!(entry.state(), Ok(EntryState::Absent));
    entry.enable().unwrap();
    assert_eq!(entry.state(), Ok(EntryState::Current));
    write_raw(&entry.approved_key, &entry.name, REG_BINARY, &[3, 0, 0, 0]).unwrap();
    assert_eq!(entry.state(), Ok(EntryState::SwitchedOff));
    entry.enable().unwrap();
    assert_eq!(entry.state(), Ok(EntryState::Current));
    entry.disable().unwrap();
    assert_eq!(entry.state(), Ok(EntryState::Absent));
    entry.disable().unwrap();
    delete_tree(&entry.run_key);
    delete_tree(&entry.approved_key);
    delete_tree(&root);
}
