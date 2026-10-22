use super::*;
use std::ffi::OsStr;
use std::io::Write as _;

#[test]
fn should_free_console_by_role_and_count() {
    assert!(should_free_console(Some(OsStr::new("tray")), 1));
    assert!(should_free_console(Some(OsStr::new("tray")), 0));
    assert!(!should_free_console(Some(OsStr::new("tray")), 2));
    assert!(!should_free_console(Some(OsStr::new("run")), 1));
    assert!(!should_free_console(None, 1));
}

/// Double-click on the .exe: argv is just the program name; the role is taken after the default
/// subcommand is substituted, and a build with the tray releases the console, a build without the tray does not.
#[test]
fn bare_exe_role_follows_default_subcommand() {
    let args = crate::cli::args::with_default_subcommand(
        [std::ffi::OsString::from("meshtown.exe")],
        cfg!(feature = "gui"),
    );
    let role = args.get(1).map(|r| r.as_os_str());
    assert_eq!(should_free_console(role, 1), cfg!(feature = "gui"));
}

/// A bare launch of a build without a tray backend takes the `run` role: neither the console is freed nor
/// stdio redirected. (`process_role` itself reads the real argv, which the test runner fills, so the role is
/// derived here from the same substitution it uses.)
#[test]
fn role_is_run_when_no_tray() {
    let args =
        crate::cli::args::with_default_subcommand([std::ffi::OsString::from("meshtown")], false);
    let role = args.get(1).map(|r| r.as_os_str());

    assert_eq!(role, Some(OsStr::new("run")));
    assert!(
        !should_free_console(role, 1),
        "a run role keeps the console"
    );
    assert!(
        !should_redirect_stdio(role, false),
        "a run role keeps stdio"
    );
}

#[test]
fn open_log_rotates() {
    let dir = std::env::temp_dir().join(format!("mesh-console-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    const NAME: &str = "meshtown-tray.log";
    let old = format!("{NAME}.1");

    open_log(&dir, NAME).unwrap().write_all(b"a").unwrap();
    open_log(&dir, NAME).unwrap().write_all(b"b").unwrap();
    assert_eq!(read(NAME), "b");
    assert_eq!(read(&old), "a");

    open_log(&dir, NAME).unwrap();
    assert_eq!(read(&old), "b");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn should_redirect_stdio_by_role_and_terminal() {
    assert!(should_redirect_stdio(Some(OsStr::new("tray")), false));
    assert!(!should_redirect_stdio(Some(OsStr::new("tray")), true));
    assert!(!should_redirect_stdio(Some(OsStr::new("run")), false));
    assert!(!should_redirect_stdio(None, false));
}

/// Launch from the app bundle without arguments: argv is one full path to the binary inside `MeshTown.app`; the role
/// after the default subcommand is substituted redirects stdio exactly in the builds where that
/// subcommand is `tray` (a build with the tray).
#[test]
fn bundle_argv_role_follows_default_subcommand() {
    let args = crate::cli::args::with_default_subcommand(
        [std::ffi::OsString::from(
            "/Applications/MeshTown.app/Contents/MacOS/meshtown",
        )],
        cfg!(feature = "gui"),
    );
    let role = args.get(1).map(|r| r.as_os_str());
    assert_eq!(should_redirect_stdio(role, false), cfg!(feature = "gui"));
}

#[test]
fn read_tail_keeps_last_bytes() {
    let dir = std::env::temp_dir().join(format!("mesh-tail-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tail.log");
    let data: Vec<u8> = (0..10_000u32).map(|i| b'a' + (i % 26) as u8).collect();
    std::fs::write(&path, &data).unwrap();

    let tail = read_tail(&path, 8192).unwrap();

    assert_eq!(tail.len(), 8192);
    assert_eq!(tail.as_bytes(), &data[10_000 - 8192..]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_tail_missing_file_is_none() {
    let path = std::env::temp_dir().join("mesh-tail-no-such-file.log");
    assert_eq!(read_tail(&path, 8192), None);
}
