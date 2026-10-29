use std::path::Path;

use super::*;
use crate::autostart::backend::EntryState;

const EXE: &str = r"C:\Program Files\Mesh Town\meshtown.exe";
const ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const DISABLED: [u8; 12] = [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[test]
fn run_value_quotes_path() {
    // given: a path with spaces
    let exe = Path::new(EXE);
    // when / then
    assert_eq!(
        run_value(exe),
        r#""C:\Program Files\Mesh Town\meshtown.exe" tray"#
    );
}

#[test]
fn state_absent_current_stale() {
    // given
    let exe = Path::new(EXE);
    let current = run_value(exe);
    let moved = run_value(Path::new(r"D:\apps\meshtown.exe"));
    // when / then
    assert_eq!(run_entry_state(None, None, exe), EntryState::Absent);
    assert_eq!(
        run_entry_state(Some(&current), None, exe),
        EntryState::Current
    );
    assert_eq!(
        run_entry_state(Some(&moved), None, exe),
        EntryState::Stale,
        "other exe path"
    );
    assert_eq!(
        run_entry_state(Some("meshtown.exe"), None, exe),
        EntryState::Stale,
        "other text"
    );
}

#[test]
fn approved_odd_byte_is_switched_off() {
    // given
    let exe = Path::new(EXE);
    let current = run_value(exe);
    // when / then
    assert!(approved_disabled(&DISABLED), "0x03 means switched off");
    assert!(approved_disabled(&[1]), "any odd first byte");
    assert_eq!(
        run_entry_state(Some(&current), Some(&DISABLED), exe),
        EntryState::SwitchedOff
    );
    assert_eq!(
        run_entry_state(Some("stale"), Some(&DISABLED), exe),
        EntryState::SwitchedOff,
        "switched off wins over stale"
    );
}

#[test]
fn approved_empty_or_even_keeps_value_state() {
    // given
    let exe = Path::new(EXE);
    let current = run_value(exe);
    // when / then
    assert!(!approved_disabled(&[]), "empty is not switched off");
    assert!(!approved_disabled(&ENABLED), "0x02 means enabled");
    assert_eq!(
        run_entry_state(Some(&current), Some(&[]), exe),
        EntryState::Current
    );
    assert_eq!(
        run_entry_state(Some(&current), Some(&ENABLED), exe),
        EntryState::Current
    );
    assert_eq!(
        run_entry_state(Some("stale"), Some(&ENABLED), exe),
        EntryState::Stale
    );
    assert_eq!(
        run_entry_state(None, Some(&DISABLED), exe),
        EntryState::Absent,
        "no value, nothing to switch off"
    );
}

#[test]
fn keys_and_value_name_are_fixed() {
    assert_eq!(RUN_KEY, r"Software\Microsoft\Windows\CurrentVersion\Run");
    assert_eq!(
        APPROVED_KEY,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    );
    assert_eq!(VALUE_NAME, "Mesh Town");
}
