//! The `password reset` and `open` subcommands of the real binary against a real listener
//! (`web::start_web_page` on loopback). The listener's token directory is the settings file directory, as for the node.
//! Tokens do not get into the `assert!` texts.

#[path = "common/web_auth.rs"]
mod auth_common;
#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/web_listener.rs"]
mod common;

use std::path::{Path, PathBuf};

use meshtown_core::web_gui::{WebHandle, DEFAULT_DRAIN};

use auth_common::{file_token, write_password, PASSWORD_OK};
use cli_node::{cli_in as mesh, free_port, stderr as err, stdout as out};
use common::{cleanup, run_config, scratch, start_web};

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

fn password_file(dir: &Path) -> PathBuf {
    dir.join("web-password")
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
    assert!(
        text.lines().all(|l| l.starts_with("http://")),
        "stdout holds more than addresses: {text}"
    );
    assert!(
        err(&o).contains("10 minutes"),
        "no validity period: {}",
        err(&o)
    );
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
        err(&o).contains("Mesh Town isn't running"),
        "stderr: {}",
        err(&o)
    );
    assert!(out(&o).is_empty(), "stdout: {}", out(&o));
    assert!(
        !password_file(&dir).exists(),
        "password file is still there"
    );
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_without_node_and_without_file_is_error() {
    let dir = scratch("cli-reset-nofile");
    let o = mesh(&["password", "reset"], &dir, free_port());
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    assert!(!out(&o).contains("Password reset"), "stdout: {}", out(&o));
    let text = err(&o);
    assert!(
        text.contains("nothing to reset") && text.contains("--config"),
        "stderr: {text}"
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
async fn open_print_shows_loopback_and_keeps_password() {
    let rig = rig("cli-open-print").await;
    let o = mesh(&["open", "--print"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(0), "stderr: {}", err(&o));
    let text = out(&o);
    assert!(text.contains("127.0.0.1"), "no loopback address: {text}");
    assert!(
        text.lines().all(|l| l.starts_with("http://")),
        "stdout holds more than addresses: {text}"
    );
    assert!(err(&o).contains("page password"), "stderr: {}", err(&o));
    assert!(
        password_file(&rig.dir).exists(),
        "password file was removed"
    );
    assert!(
        !text.contains(&file_token(&rig.dir)),
        "permanent token in the output"
    );
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn open_without_node_fails_with_hint() {
    let dir = scratch("cli-open-off");
    let o = mesh(&["open", "--print"], &dir, free_port());
    assert_eq!(o.status.code(), Some(3), "stdout: {}", out(&o));
    assert!(err(&o).contains("--web-port"), "stderr: {}", err(&o));
    assert!(!out(&o).contains("http://"), "stdout: {}", out(&o));
    cleanup(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn open_without_token_hints_sudo() {
    let rig = rig("cli-open-notoken").await;
    std::fs::remove_file(rig.dir.join("web-token")).expect("token removal");
    let o = mesh(&["open", "--print"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    let text = err(&o);
    assert!(
        text.contains("web-token") && text.contains("sudo"),
        "stderr: {text}"
    );
    assert!(
        !out(&o).contains("http://"),
        "an address was printed on refusal"
    );
    finish(rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn open_with_wrong_token_is_refused() {
    let rig = rig("cli-open-403").await;
    std::fs::write(rig.dir.join("web-token"), "wrong-token\n").expect("token substitution");
    let o = mesh(&["open", "--print"], &rig.dir, rig.port);
    assert_eq!(o.status.code(), Some(1), "stdout: {}", out(&o));
    let text = err(&o);
    assert!(
        text.contains("refused the sign-in link") && !text.contains("sudo"),
        "stderr: {text}"
    );
    assert!(
        !out(&o).contains("http://"),
        "an address was printed on refusal"
    );
    finish(rig).await;
}
