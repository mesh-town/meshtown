//! Writing commands in a settings directory that others can enter: the warning goes to stderr and stdout stays
//! empty (stdout of these commands is data), and a seed command that finds a silent foreign listener on the page
//! port still ends quickly and leaves the file as it was.
#![cfg(unix)]

#[path = "common/cli_node.rs"]
mod cli_node;

use std::path::PathBuf;

use meshtown_core::config::config_file::{new_secret, save_config_file, Settings};

use cli_node::{cli_in as mesh, free_port, scratch_dir, stderr as err, stdout as out};

const WARNING: &str = "accessible to more than its owner";

/// A fresh directory with mode 0755, optionally with a settings file inside.
fn loose_dir(tag: &str, with_file: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir(tag);
    if with_file {
        let secret = new_secret().expect("secret");
        let file = dir.join("config.json");
        save_config_file(&file, None, 1, &secret, &Settings::default()).expect("settings file");
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("mode");
    dir
}

/// Removes `socks.ifaces` and `web.ifaces` from the settings file, as in a file written by an older version.
fn drop_iface_keys(file: &std::path::Path) {
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    for section in ["socks", "web"] {
        doc["config"][section]
            .as_object_mut()
            .unwrap()
            .remove("ifaces");
    }
    std::fs::write(file, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

fn assert_warns_on_stderr_only(o: &std::process::Output) {
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(o));
    assert_eq!(out(o), "", "stdout must stay empty");
    assert!(err(o).contains(WARNING), "stderr: {}", err(o));
}

#[test]
fn write_into_loose_dir_keeps_stdout_empty() {
    let dir = loose_dir("loose-mode", true);
    let o = mesh(&["access", "page", "localhost"], &dir, free_port());
    assert_warns_on_stderr_only(&o);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn seed_into_loose_dir_keeps_stdout_empty() {
    let dir = loose_dir("loose-seed", false);
    let o = mesh(
        &["access", "ifaces", "br0", "--if-unset"],
        &dir,
        free_port(),
    );
    assert_warns_on_stderr_only(&o);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_set_into_loose_dir_keeps_stdout_empty() {
    let dir = loose_dir("loose-set", true);
    let o = mesh(
        &["config", "set", "probe.interval_sec", "20"],
        &dir,
        free_port(),
    );
    assert_warns_on_stderr_only(&o);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file with no interface keys and a silent foreign listener on the page port: the command gives up with
/// code 1 within seven seconds and does not touch the file.
#[test]
fn ifaces_if_unset_on_keyless_file_with_silent_port_fails_fast() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    let port = busy.local_addr().unwrap().port();
    let dir = loose_dir("loose-silent", true);
    drop_iface_keys(&dir.join("config.json"));
    let before = std::fs::read(dir.join("config.json")).unwrap();
    let started = std::time::Instant::now();

    let o = mesh(&["access", "ifaces", "br0", "--if-unset"], &dir, port);

    assert_eq!(o.status.code(), Some(1), "stderr: {}", err(&o));
    // The client waits 5 s for a reply; the margin covers process start under a loaded machine.
    assert!(started.elapsed() < std::time::Duration::from_secs(7));
    assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
    drop(busy);
    let _ = std::fs::remove_dir_all(&dir);
}
