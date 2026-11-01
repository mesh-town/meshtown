//! `meshtown --version` of the real binary: the package version, then the short commit hash in parentheses when the
//! source tree is a git checkout.

use std::process::Command;

fn expected_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty());
    match hash {
        Some(h) => format!("meshtown {version} ({h})"),
        None => format!("meshtown {version}"),
    }
}

#[test]
fn version_flag_prints_package_version_and_commit() {
    let out = Command::new(env!("CARGO_BIN_EXE_meshtown"))
        .arg("--version")
        .output()
        .expect("start meshtown");

    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected_line());
}
