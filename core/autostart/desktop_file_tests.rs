use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

/// An absolute directory name on the host running the tests.
fn root(name: &str) -> PathBuf {
    let base = if cfg!(windows) { r"C:\" } else { "/" };
    Path::new(base).join(name)
}

fn entry_in(dir: &Path) -> PathBuf {
    dir.join("autostart").join("meshtown.desktop")
}

#[test]
fn path_prefers_absolute_xdg() {
    // given
    let env = env_of(&[("XDG_CONFIG_HOME", root("xdg")), ("HOME", root("home"))]);
    // when / then
    assert_eq!(desktop_entry_path(&env), Ok(entry_in(&root("xdg"))));
}

#[test]
fn path_ignores_relative_xdg() {
    // given: a relative XDG_CONFIG_HOME next to HOME, and next to nothing
    let with_home = env_of(&[
        ("XDG_CONFIG_HOME", "rel/conf".into()),
        ("HOME", root("home")),
    ]);
    let alone = env_of(&[("XDG_CONFIG_HOME", "rel/conf".into())]);
    // when / then
    assert_eq!(
        desktop_entry_path(&with_home),
        Ok(entry_in(&root("home").join(".config")))
    );
    assert!(
        desktop_entry_path(&alone).is_err(),
        "a relative value is not a directory to use"
    );
}

#[test]
fn path_falls_back_to_home() {
    let env = env_of(&[("HOME", root("home"))]);
    assert_eq!(
        desktop_entry_path(&env),
        Ok(entry_in(&root("home").join(".config")))
    );
}

#[test]
fn path_without_env_fails() {
    let env = env_of(&[]);
    assert_eq!(
        desktop_entry_path(&env),
        Err("neither XDG_CONFIG_HOME nor HOME is set".to_string())
    );
}

#[test]
fn path_treats_empty_as_unset() {
    // given: empty values
    let only_empty = env_of(&[("XDG_CONFIG_HOME", "".into()), ("HOME", "".into())]);
    let empty_xdg = env_of(&[("XDG_CONFIG_HOME", "".into()), ("HOME", root("home"))]);
    // when / then
    assert!(desktop_entry_path(&only_empty).is_err(), "both empty");
    assert_eq!(
        desktop_entry_path(&empty_xdg),
        Ok(entry_in(&root("home").join(".config")))
    );
}

#[test]
fn exec_escapes_specials() {
    // given: (argument, text after quoting and the string-value escape)
    let cases = [
        ("/usr/bin/meshtown", r#""/usr/bin/meshtown""#),
        ("/opt/my app/mesh", r#""/opt/my app/mesh""#),
        (r#"a"b"#, r#""a\\"b""#),
        ("a`b", r#""a\\`b""#),
        ("a$b", r#""a\\$b""#),
        (r"a\b", r#""a\\\\b""#),
        ("100%", r#""100%%""#),
    ];
    for (arg, expected) in cases {
        // when / then
        assert_eq!(
            desktop_exec_arg(arg).as_deref(),
            Ok(expected),
            "argument {arg:?}"
        );
    }
}

#[test]
fn exec_rejects_newline() {
    assert!(desktop_exec_arg("a\nb").is_err(), "line feed");
    assert!(desktop_exec_arg("a\rb").is_err(), "carriage return");
}

#[test]
fn entry_golden() {
    // given
    let exe = Path::new("/opt/mesh town/meshtown-linux");
    // when
    let text = desktop_entry(exe).unwrap();
    // then
    let expected = r#"[Desktop Entry]
Type=Application
Name=Mesh Town
Comment=Start Mesh Town at login
Exec="/opt/mesh town/meshtown-linux" tray
Terminal=false
X-GNOME-Autostart-enabled=true
"#;
    assert_eq!(text, expected);
}

#[cfg(unix)]
#[test]
fn entry_rejects_non_utf8_path() {
    use std::os::unix::ffi::OsStrExt;
    let exe = Path::new(std::ffi::OsStr::from_bytes(b"/opt/\xff/meshtown"));
    assert!(desktop_entry(exe).is_err());
}

#[test]
fn hidden_or_gnome_false_is_switched_off() {
    // given
    let own = desktop_entry(Path::new("/opt/meshtown")).unwrap();
    let hidden = format!("{own}Hidden=true\n");
    let gnome_off = own.replace(
        "X-GNOME-Autostart-enabled=true",
        "  X-GNOME-Autostart-enabled=false  ",
    );
    // when / then
    assert!(desktop_switched_off(&hidden), "Hidden=true");
    assert!(
        desktop_switched_off(&gnome_off),
        "GNOME flag false, with spaces"
    );
    assert!(!desktop_switched_off(&own), "own text is not switched off");
    assert!(!desktop_switched_off("Hidden=false\n"), "Hidden=false");
}

fn env_of(pairs: &[(&str, PathBuf)]) -> impl Fn(&str) -> Option<OsString> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone().into_os_string()))
        .collect();
    move |name| map.get(name).cloned()
}
