//! The `password reset` and `access` subcommands of the real binary against a real listener
//! (`web::start_web_page` on loopback). The listener's token directory is the settings file directory, as for the node.
//! Tokens do not get into the `assert!` texts.

#[path = "common/web_auth.rs"]
mod auth_common;
#[path = "common/web_listener.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use meshtown_core::web_gui::{WebHandle, DEFAULT_DRAIN};
use serde_json::Value;

use auth_common::{file_token, write_password, PASSWORD_OK};
use common::{api, cleanup, run_config, scratch, start_web};

struct Rig {
    dir: PathBuf,
    handle: WebHandle,
    port: u16,
}

async fn rig(tag: &str) -> Rig {
    let dir = scratch(tag);
    let run = run_config(&dir);
    write_password(&dir, PASSWORD_OK, 1000);
    let (handle, _node) = start_web(&dir, &run).await;
    let port = handle.shared.port();
    Rig { dir, handle, port }
}

async fn finish(rig: Rig) {
    rig.handle.shutdown(DEFAULT_DRAIN).await;
    cleanup(&rig.dir);
}

/// A port nobody is listening on right now.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    l.local_addr().unwrap().port()
}

fn mesh(args: &[&str], dir: &Path, port: u16) -> Output {
    Command::new(env!("CARGO_BIN_EXE_meshtown"))
        .args(args)
        .arg("--config")
        .arg(dir.join("config.json"))
        .args(["--web-port", &port.to_string()])
        .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE")
        .output()
        .expect("start meshtown")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn password_file(dir: &Path) -> PathBuf {
    dir.join("web-password")
}

fn config_json(dir: &Path) -> Value {
    let text = std::fs::read_to_string(dir.join("config.json")).expect("config.json");
    serde_json::from_str(&text).expect("JSON")
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_live_prints_temp_url_and_removes_file() {
    let rig = rig("cli-reset-live").await;
    let o = mesh(&["password", "reset"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    let text = out(&o);
    assert!(
        text.contains("?token="),
        "no address with a temporary token"
    );
    assert!(text.contains("10 minutes"), "no validity period: {text}");
    assert!(
        !password_file(&rig.dir).exists(),
        "password file is still there"
    );
    assert!(
        !text.contains(&file_token(&rig.dir)),
        "permanent token in the output"
    );
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_without_node_removes_file_exit_0() {
    let dir = scratch("cli-reset-off");
    write_password(&dir, PASSWORD_OK, 1000);
    let o = mesh(&["password", "reset"], &dir, free_port());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        out(&o).contains("The node isn't running"),
        "stdout: {}",
        out(&o)
    );
    assert!(
        !password_file(&dir).exists(),
        "password file is still there"
    );
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_with_wrong_token_exit_1_file_kept() {
    let rig = rig("cli-reset-403").await;
    std::fs::write(rig.dir.join("web-token"), "wrong-token\n").expect("token substitution");
    let o = mesh(&["password", "reset"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    assert!(err(&o).starts_with("Error"), "stderr: {}", err(&o));
    assert!(
        password_file(&rig.dir).exists(),
        "password file was deleted"
    );
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn access_live_localhost_is_immediate() {
    let rig = rig("cli-access-now").await;
    let o = mesh(&["access", "localhost"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        out(&o).contains("takes effect immediately"),
        "stdout: {}",
        out(&o)
    );
    let token = file_token(&rig.dir);
    let (_, st) = api(rig.port, &token, "GET", "/api/auth/status", None);
    assert_eq!(st["access"], "localhost", "{st}");
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn access_live_localnet_needs_restart() {
    let rig = rig("cli-access-later").await;
    let o = mesh(&["access", "localnet"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        out(&o).contains("after the node restarts"),
        "stdout: {}",
        out(&o)
    );
    let token = file_token(&rig.dir);
    let (_, st) = api(rig.port, &token, "GET", "/api/auth/status", None);
    assert_eq!(st["access"], "localnet", "{st}");
    assert_eq!(st["listening"], "localhost", "{st}");
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn access_without_node_writes_field() {
    let dir = scratch("cli-access-off");
    let _ = run_config(&dir);
    let before = config_json(&dir);
    let o = mesh(&["access", "localhost"], &dir, free_port());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    assert!(
        out(&o).contains("applies at startup"),
        "stdout: {}",
        out(&o)
    );
    let after = config_json(&dir);
    assert_eq!(after["config"]["web"]["access"], "localhost", "{after}");
    assert!(
        after["rev"].as_u64() > before["rev"].as_u64(),
        "rev did not grow: {before} -> {after}"
    );
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn access_without_node_and_without_file_is_error() {
    let dir = scratch("cli-access-nofile");
    let o = mesh(&["access", "localhost"], &dir, free_port());
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    assert!(err(&o).contains("no settings file"), "stderr: {}", err(&o));
    cleanup(&dir);
}

#[test]
fn access_bogus_value_is_cli_error() {
    let dir = scratch("cli-access-bogus");
    let o = mesh(&["access", "everyone"], &dir, free_port());
    assert_eq!(o.status.code(), Some(2), "stdout: {}", out(&o));
    let text = err(&o);
    assert!(
        text.contains("localhost") && text.contains("localnet"),
        "stderr: {text}"
    );
    cleanup(&dir);
}
