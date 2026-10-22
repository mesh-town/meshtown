//! Entry URLs of the node page: without a password the URL carries a temporary token, not the permanent one.
//! Tokens are never printed and don't get into `assert!` texts.

#[path = "common/web_listener.rs"]
mod common;

use meshtown_core::web_gui::{DEFAULT_DRAIN, TOKEN_FILE_NAME};

use common::{api, cleanup, run_config, scratch, start_web};

mod entry_urls {
    use super::*;

    mod without_password {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn temp_token_differs_from_file_and_fails_as_header() {
            let dir = scratch("entry-urls");
            let run = run_config(&dir);
            let (handle, _node) = start_web(&dir, &run).await;
            let port = handle.shared.port();
            let file_token = std::fs::read_to_string(dir.join(TOKEN_FILE_NAME))
                .expect("web-token must be readable")
                .trim()
                .to_string();

            let urls = handle.shared.issue_entry_urls();
            assert_eq!(urls.len(), 1, "with localhost there is one URL");
            let prefix = format!("http://127.0.0.1:{port}/?token=");
            let temp = urls[0]
                .strip_prefix(&prefix)
                .expect("entry URL: 127.0.0.1:<port>/?token=T")
                .to_string();
            assert_eq!(temp.chars().count(), 52);
            assert!(temp != file_token, "the permanent token got into the URL");

            let (status, _) = api(port, &temp, "GET", "/api/state", None);
            assert_eq!(status, 401, "a temporary token must not pass as a header");
            let (status, _) = api(port, &file_token, "GET", "/api/state", None);
            assert_eq!(status, 200, "the permanent token from the file must pass");

            handle.shutdown(DEFAULT_DRAIN).await;
            cleanup(&dir);
        }
    }
}
