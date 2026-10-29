use super::*;

fn temp_home(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("meshtown-autostart-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn enable_then_disable_in_temp_home() {
    let home = temp_home("mac");
    let entry = entry_in(&home, None, Path::new("/opt/mt/meshtown-macos"));

    assert_eq!(entry.state(), Ok(EntryState::Absent));
    entry.enable().unwrap();
    let file = home.join("Library/LaunchAgents/local.meshtown.plist");
    assert!(file.is_file(), "the agent file is written");
    assert_eq!(entry.state(), Ok(EntryState::Current));
    entry.disable().unwrap();
    assert!(!file.exists(), "the agent file is removed");
    assert_eq!(entry.state(), Ok(EntryState::Absent));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn bundle_uses_its_identifier_as_label() {
    let home = Path::new("/Users/example");
    let app = Path::new("/Applications/Mesh Town.app");

    let entry = entry_in(
        home,
        Some((app, "app.example.tray")),
        Path::new("/x/meshtown"),
    );

    assert_eq!(
        entry.path,
        Path::new("/Users/example/Library/LaunchAgents/app.example.tray.plist")
    );
    assert!(entry.text.contains("/Applications/Mesh Town.app"));
}

#[test]
fn static_handle_reads_state_without_writing() {
    let backend: &'static dyn AutostartBackend = &AUTOSTART;

    assert!(backend.state().is_ok(), "reading the login entry succeeds");
}

#[test]
fn install_registers_the_static_handle() {
    meshtown_core::autostart::install(&AUTOSTART);

    assert!(meshtown_core::autostart::installed().is_some());
}
