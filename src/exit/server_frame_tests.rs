//! Tests for `read_request_raw` (`server.rs`): the request frame is read whole or the result is `Err(())`.
//! The stream is ready-made bytes and an end (`Cursor`); the handler on the mem stack is in `server_service_tests.rs`.

use super::*;
use crate::exit::frame::encode_exit_request;

mod read_request_raw_frame {
    use super::*;

    #[tokio::test]
    async fn empty_stream_is_err() {
        assert_eq!(raw_of(&[]).await, Err(()));
    }

    #[tokio::test]
    async fn cut_inside_head_is_err() {
        assert_eq!(raw_of(&[EXIT_PROTOCOL_VERSION]).await, Err(()), "truncated at 1 byte");
    }

    #[tokio::test]
    async fn cut_after_head_is_err() {
        assert_eq!(
            raw_of(&[EXIT_PROTOCOL_VERSION, 5]).await,
            Err(()),
            "the header is there, the name and port are not"
        );
    }

    #[tokio::test]
    async fn full_frame_is_returned_as_is() {
        let frame = request("example.com", 443);

        assert_eq!(raw_of(&frame).await, Ok(frame));
    }
}

/// Frame bytes as `read_request_raw` parses them: the stream is ready-made bytes and an end.
async fn raw_of(bytes: &[u8]) -> std::result::Result<Vec<u8>, ()> {
    let mut recv: BoxRecv = Box::new(std::io::Cursor::new(bytes.to_vec()));
    read_request_raw(&mut recv).await
}

/// A request frame `[EXIT_PROTOCOL_VERSION, name length, name, port]` — the same one `ask_exit_to_connect` sends.
fn request(domain: &str, port: u16) -> Vec<u8> {
    encode_exit_request(domain, port).expect("the frame encodes")
}
