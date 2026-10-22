//! Tests of the node refusal reason (`start_note.rs`): log parsing and menu lines.

use super::*;
use crate::app::tray_texts::START_TEXT;
use crate::app::view::{node_status_menu_line, PageState, RouteLine, View};

fn failed(code: Option<i32>, has_node_log: bool) -> StartNote {
    StartNote::Failed { code, has_node_log }
}

#[tokio::test]
async fn real_bind_error_is_proxy_was_busy() {
    let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let e = meshtown::proxy::bind_proxy_listener(port, meshtown::config_file::ListenAccess::Localhost).await.unwrap_err();
    let log = format!(
        "+ 0.1s [node] start\n{}\n",
        crate::cli_errors::error_report(&e)
    );
    assert_eq!(
        start_note_from(Some(1), Some(&log)),
        StartNote::ProxyWasBusy(port)
    );
}

#[test]
fn last_error_line_wins() {
    let log = "+ 1.0s restart failed: port 1080 is in use\n\
               Error: port 1080 for the browser proxy is in use by another program (Address already in use (os error 48)).\n\
               Close it or start the node on another port: meshtown run --socks-port 1081\n";
    assert_eq!(
        start_note_from(Some(1), Some(log)),
        StartNote::ProxyWasBusy(1080)
    );
}

#[test]
fn other_error_gives_reason() {
    let log = "Error: the Mesh Town network key is not set: --key is required\n";
    assert_eq!(
        start_note_from(Some(1), Some(log)),
        StartNote::FailedWith("the Mesh Town network key is not set: --key is required".into())
    );
}

#[test]
fn no_error_line_gives_code() {
    assert_eq!(
        start_note_from(Some(1), Some("+ 0.1s [node] start\n")),
        failed(Some(1), true)
    );
    assert_eq!(start_note_from(Some(1), None), failed(Some(1), false));
}

#[test]
fn broken_port_number_gives_reason() {
    let log = "Error: port abc for the browser proxy is in use by another program\n";
    assert_eq!(
        start_note_from(Some(1), Some(log)),
        StartNote::FailedWith("port abc for the browser proxy is in use by another program".into())
    );
}

#[test]
fn proxy_busy_texts() {
    let note = StartNote::ProxyBusy(1080);
    assert_eq!(
        start_note_line(&note).unwrap(),
        "Mesh Town isn't running: another program uses proxy port 1080"
    );
    assert_eq!(
        start_note_hint(&note).unwrap(),
        format!("Close that program, then click \"{START_TEXT}\"")
    );
}

#[test]
fn proxy_was_busy_texts() {
    let note = StartNote::ProxyWasBusy(1081);
    assert_eq!(
        start_note_line(&note).unwrap(),
        "Mesh Town isn't running: proxy port 1081 was in use"
    );
    assert_eq!(
        start_note_hint(&note).unwrap(),
        format!("Click \"{START_TEXT}\" again")
    );
}

/// The first menu line about a busy page port: it is built by `view`, not `start_note`.
fn page_busy_line(port: u16) -> String {
    let view = View {
        page: PageState::PortBusy(port),
        ..View::initial()
    };
    node_status_menu_line(view, &RouteLine::Direct)
}

/// All first menu lines that name a port number.
fn port_lines(port: u16) -> [String; 3] {
    [
        start_note_line(&StartNote::ProxyBusy(port)).unwrap(),
        start_note_line(&StartNote::ProxyWasBusy(port)).unwrap(),
        page_busy_line(port),
    ]
}

#[test]
fn port_lines_fit_the_menu() {
    // The longest port has five characters; there is room in the line for it too.
    for port in [1080, 65535] {
        for line in port_lines(port) {
            let len = line.chars().count();
            assert!(len <= LINE_MAX_CHARS, "{len} chars: {line}");
        }
    }
}

#[test]
fn port_lines_keep_the_port_and_drop_the_proxy_words() {
    for note in [StartNote::ProxyBusy(1080), StartNote::ProxyWasBusy(1080)] {
        let line = start_note_line(&note).unwrap();
        assert!(line.contains("proxy port 1080"), "{line}");
        assert!(!line.contains("for the browser proxy"), "{line}");
    }
}

#[test]
fn proxy_and_page_busy_lines_name_whose_port_it_is() {
    // Before, both read "Mesh Town isn't running: port 1080 is in use by another program": from the menu
    // one couldn't tell which port was meant.
    let proxy = start_note_line(&StartNote::ProxyBusy(1080)).unwrap();
    let page = page_busy_line(1080);
    assert_ne!(proxy, page);
    assert!(proxy.contains("proxy port 1080"), "{proxy}");
    assert!(page.contains("page port 1080"), "{page}");
}

#[test]
fn failed_with_log_texts() {
    let note = failed(Some(3), true);
    assert_eq!(
        start_note_line(&note).unwrap(),
        "Mesh Town stopped with an error (code 3)"
    );
    assert_eq!(
        start_note_hint(&note).unwrap(),
        format!("Details: {NODE_LOG_NAME}, next to the settings file")
    );
}

#[test]
fn failed_without_log_has_no_hint() {
    let note = failed(Some(3), false);
    assert_eq!(
        start_note_line(&note).unwrap(),
        "Mesh Town stopped with an error (code 3)"
    );
    assert_eq!(start_note_hint(&note), None);
}

#[test]
fn failed_without_code_texts() {
    assert_eq!(
        start_note_line(&failed(None, true)).unwrap(),
        "Mesh Town stopped with an error"
    );
    assert!(start_note_hint(&failed(None, true))
        .unwrap()
        .contains(NODE_LOG_NAME));
    assert_eq!(start_note_hint(&failed(None, false)), None);
}

#[test]
fn idle_has_no_texts() {
    assert_eq!(start_note_line(&StartNote::Idle), None);
    assert_eq!(start_note_hint(&StartNote::Idle), None);
}

#[test]
fn old_error_prefix_is_ignored() {
    let log = "Warning: port 1080 for the browser proxy is in use by another program (OS).\n";
    assert_eq!(start_note_from(Some(1), Some(log)), failed(Some(1), true));
}

#[test]
fn old_port_phrase_gives_reason() {
    let log = "Error: port 1080 for the proxy (SOCKS5 and HTTP) is in use or unavailable\n";
    assert_eq!(
        start_note_from(Some(1), Some(log)),
        StartNote::FailedWith("port 1080 for the proxy (SOCKS5 and HTTP) is in use or unavailable".into())
    );
}

#[test]
fn permission_and_page_errors_give_reason() {
    for line in [
        "Error: no permission to open port 80 for the browser proxy (OS).",
        "Error: Mesh Town: the node page is unavailable, and without a network key you can only sign in through it.",
    ] {
        let log = format!("{line}\n");
        let want = line.strip_prefix("Error: ").unwrap().to_owned();
        assert_eq!(start_note_from(Some(1), Some(&log)), StartNote::FailedWith(want));
    }
}

#[test]
fn error_without_colon_gives_code() {
    let log = "Invalid network key from the settings file /c.toml: bad key\n";
    assert_eq!(start_note_from(Some(1), Some(log)), failed(Some(1), true));
}

#[test]
fn panic_line_gives_code_101() {
    let log = "thread 'main' panicked at core/main.rs:1:1:\nsomething\n";
    assert_eq!(
        start_note_from(Some(101), Some(log)),
        failed(Some(101), true)
    );
}

#[test]
fn reason_line_and_hint() {
    let note = StartNote::FailedWith("bad key".into());
    assert_eq!(start_note_line(&note).unwrap(), "Mesh Town stopped: bad key");
    assert_eq!(
        start_note_hint(&note).unwrap(),
        format!("Details: {NODE_LOG_NAME}, next to the settings file")
    );
}

#[test]
fn long_reason_cut_by_chars() {
    let note = StartNote::FailedWith("é".repeat(100));
    let line = start_note_line(&note).unwrap();
    assert_eq!(line.chars().count(), LINE_MAX_CHARS);
    assert!(line.ends_with('…'), "{line}");
}

#[test]
fn reason_at_limit_not_cut() {
    let head = "Mesh Town stopped: ";
    let fill = LINE_MAX_CHARS - head.chars().count();
    let note = StartNote::FailedWith("é".repeat(fill));
    let line = start_note_line(&note).unwrap();
    assert_eq!(line.chars().count(), LINE_MAX_CHARS);
    assert!(!line.ends_with('…'), "{line}");
}

#[test]
fn blank_reason_gives_code() {
    assert_eq!(start_note_from(Some(1), Some("Error:   \n")), failed(Some(1), true));
}

#[test]
fn advice_after_error_not_in_reason() {
    let note = start_note_from(Some(1), Some("Error: a\nadvice\n"));
    assert_eq!(note, StartNote::FailedWith("a".into()));
}
