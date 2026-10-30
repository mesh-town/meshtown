//! `network create --key-only` prints the key as the only line of stdout; warnings go to stderr.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn stdout_is_one_line_even_when_the_directory_is_wide() {
    let dir = std::env::temp_dir().join(format!("key_only_stdout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_meshtown"))
        .args(["network", "create", "--key-only", "--raw", "--config"])
        .arg(dir.join("config.json"))
        .output()
        .expect("start meshtown");
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert!(stdout.starts_with("mesh2-"), "{stdout}");
    assert!(stderr.contains("mode 0755"), "{stderr}");
}
