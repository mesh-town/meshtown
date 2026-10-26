//! Tests of `dial_peer` for UDP requests on `NetStack` over `MemNet`: what a reply code means to a UDP
//! caller and to a TCP one, and the stream to a node that publishes the port over UDP.

use std::sync::Arc;
use std::time::Duration;

use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::ports_sharing::dial::dial_peer;
use crate::ports_sharing::test_support::{
    datagram_frame, linked, scripted, serving_on, spawn_udp_echo,
};
use crate::ports_sharing::{
    dial_peer_port, port_code_text, PortDialError, PortReach, PortRequest, PortStream, Proto,
    PORT_CODE_BAD_REQUEST, PORT_CODE_BUSY, PORT_CODE_NOT_PUBLISHED, PORT_FRAME_VERSION,
};
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::{MemNet, NetStack, ServiceHandler, PORT};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEADLINE: Duration = Duration::from_secs(5);

mod old_server_answers_bad_request {
    use super::*;

    #[tokio::test]
    async fn old_server_reply_means_unsupported() {
        // given: the node answers code 6 to everything, as a node without UDP ports does
        let fx = setup(replying(PORT_CODE_BAD_REQUEST)).await;

        // when: A dials a UDP port
        let err = fx.when_dial_fails(udp(53)).await;

        // then: the class is "unsupported", with the text about an older version
        assert_eq!(err.reach, PortReach::Unsupported);
        assert_eq!(
            err.text,
            "the node does not support UDP ports (it runs an older version)"
        );
    }

    #[tokio::test]
    async fn bad_request_on_tcp_stays_failed() {
        // given: the same node
        let fx = setup(replying(PORT_CODE_BAD_REQUEST)).await;

        // when: A dials a TCP port
        let err = fx.when_dial_port_fails(53).await;

        // then: a plain failure with the text of the code
        assert_eq!(err.reach, PortReach::Failed);
        assert_eq!(err.text, port_code_text(PORT_CODE_BAD_REQUEST));
    }

    #[tokio::test]
    async fn bad_request_on_a_tcp_request_of_dial_peer_stays_failed() {
        // given: the same node
        let fx = setup(replying(PORT_CODE_BAD_REQUEST)).await;

        // when: A dials a TCP port through the shared path
        let err = fx.when_dial_fails(tcp(53)).await;

        // then: the protocol of the request, not the code alone, decides the class
        assert_eq!(err.reach, PortReach::Failed);
    }
}

mod busy_node {
    use super::*;

    #[tokio::test]
    async fn busy_reply_is_failed_with_text() {
        // given: the node answers code 7 ("too many UDP sessions")
        let fx = setup(replying(PORT_CODE_BUSY)).await;

        // when: A dials a UDP port
        let err = fx.when_dial_fails(udp(53)).await;

        // then: a failure that says why
        assert_eq!(err.reach, PortReach::Failed);
        assert_eq!(err.text, "too many UDP sessions");
    }
}

mod publishing_node {
    use super::*;

    #[tokio::test]
    async fn udp_request_opens_a_stream_that_carries_datagram_frames() {
        // given: B publishes a UDP echo port to everyone
        let echo = spawn_udp_echo().await;
        let fx = setup(serving_on(Proto::Udp, echo)).await;

        // when: A dials it and writes one framed datagram
        let mut stream = fx.when_dial(udp(echo)).await;
        stream
            .send
            .write_all(&datagram_frame(b"hello"))
            .await
            .unwrap();

        // then: the same frame comes back
        let mut buf = vec![0u8; datagram_frame(b"hello").len()];
        within("echo frame", stream.recv.read_exact(&mut buf))
            .await
            .unwrap();
        assert_eq!(buf, datagram_frame(b"hello"));
    }

    #[tokio::test]
    async fn udp_request_for_a_tcp_only_port_is_not_opened() {
        // given: B publishes the number over TCP only
        let fx = setup(serving_on(Proto::Tcp, 53)).await;

        // when: A dials the number as UDP
        let err = fx.when_dial_fails(udp(53)).await;

        // then: "not opened", the code of an unpublished port
        assert_eq!(err.reach, PortReach::NotOpened);
        assert_eq!(err.text, port_code_text(PORT_CODE_NOT_PUBLISHED));
    }
}

fn udp(port: u16) -> PortRequest {
    PortRequest {
        proto: Proto::Udp,
        port,
    }
}

fn tcp(port: u16) -> PortRequest {
    PortRequest {
        proto: Proto::Tcp,
        port,
    }
}

/// The node that answers every request with `[1, code]`.
fn replying(code: u8) -> Arc<dyn ServiceHandler> {
    scripted(&[PORT_FRAME_VERSION, code])
}

/// A (seed 1) dials B (seed 2, `peer` on `PORT`); B is a member of A.
struct Fixture {
    stack: NetStack,
    pool: PeerConnPool,
    members: Members,
    _keep: (Arc<MemNet>, NetStack),
}

async fn setup(peer: Arc<dyn ServiceHandler>) -> Fixture {
    let net = MemNet::new();
    let (a, b) = linked(&net, peer);
    let members = Members::default();
    members.add_verified_conn(pid(2));
    Fixture {
        stack: a,
        pool: PeerConnPool::new(PORT),
        members,
        _keep: (net, b),
    }
}

impl Fixture {
    async fn dial(&self, req: PortRequest) -> Result<PortStream, PortDialError> {
        let dialing = dial_peer(
            &self.stack,
            &self.pool,
            &self.members,
            pid(2),
            req,
            DEADLINE,
        );
        let dialed = tokio::time::timeout(DEADLINE + Duration::from_secs(1), dialing).await;
        dialed.unwrap_or_else(|_| panic!("dial_peer ignored its deadline"))
    }

    async fn when_dial(&self, req: PortRequest) -> PortStream {
        let dialed = self.dial(req).await;
        dialed.unwrap_or_else(|e| panic!("the dial was refused: {}", e.text))
    }

    async fn when_dial_fails(&self, req: PortRequest) -> PortDialError {
        self.dial(req)
            .await
            .err()
            .expect("the dial was expected to fail")
    }

    async fn when_dial_port_fails(&self, port: u16) -> PortDialError {
        let dialing = dial_peer_port(
            &self.stack,
            &self.pool,
            &self.members,
            pid(2),
            port,
            DEADLINE,
        );
        dialing.await.err().expect("the dial was expected to fail")
    }
}
