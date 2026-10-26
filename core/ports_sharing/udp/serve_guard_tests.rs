//! Tests of the small guards of the publishing loop: which socket errors only echo a closed port, and the
//! bound on the frame buffer.

use std::io::{self, ErrorKind};
use std::time::Duration;

use tokio::io::{duplex, split, AsyncWriteExt};

use crate::ports_sharing::test_support::free_udp_port;
use crate::ports_sharing::udp::UdpSession;
use crate::transport::net::facade::test_support::within;

use super::*;

mod closed_port_echo {
    use super::*;

    #[test]
    fn refused_reset_and_aborted_are_echoes() {
        for kind in [
            ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
        ] {
            assert!(is_port_closed_echo(&io::Error::from(kind)), "{kind:?}");
        }
    }

    #[test]
    fn other_errors_are_not_echoes() {
        for kind in [
            ErrorKind::BrokenPipe,
            ErrorKind::Other,
            ErrorKind::PermissionDenied,
        ] {
            assert!(!is_port_closed_echo(&io::Error::from(kind)), "{kind:?}");
        }
    }
}

mod frame_buffer {
    use super::*;

    #[tokio::test]
    async fn oversized_header_ends_the_session() {
        // given: a session towards a closed port
        let (near, mut remote) = duplex(2 * 1024 * 1024);
        let (recv, send) = split(near);
        let session = UdpSession::new(
            Box::new(send),
            Box::new(recv),
            Duration::from_secs(60),
            None,
        );
        let mut serving = crate::node::output::spawn(serve_udp_stream(session, free_udp_port()));

        // when: the stream announces a frame of 1 MiB and sends more than the limit of it
        let mut bytes = (1024u32 * 1024).to_be_bytes().to_vec();
        bytes.extend(vec![0u8; 4 + RECV_BUF]);
        remote.write_all(&bytes).await.unwrap();

        // then: the serving task ends
        let ended = within("serving ends", &mut serving).await;
        assert!(ended.is_ok(), "the serving task panicked");
    }
}
