use super::*;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("meshtown-autostart-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn enable_then_disable_in_temp_xdg_home() {
    let xdg = temp_dir("xdg");
    let value = xdg.clone().into_os_string();
    let var = move |name: &str| (name == "XDG_CONFIG_HOME").then(|| value.clone());
    let entry = entry_in(&var, Path::new("/opt/mt/meshtown-linux")).unwrap();

    assert_eq!(entry.state(), Ok(EntryState::Absent));
    entry.enable().unwrap();
    let file = xdg.join("autostart/meshtown.desktop");
    assert!(file.is_file(), "the desktop file is written");
    assert_eq!(entry.state(), Ok(EntryState::Current));
    entry.disable().unwrap();
    assert!(!file.exists(), "the desktop file is removed");
    assert_eq!(entry.state(), Ok(EntryState::Absent));
    let _ = std::fs::remove_dir_all(&xdg);
}

#[test]
fn no_home_is_an_error() {
    let var = |_: &str| None;

    assert!(entry_in(&var, Path::new("/opt/mt/meshtown-linux")).is_err());
}
