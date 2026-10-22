//! Tests for `exit/server.rs`: the handler's low-latency socket setting. The domain address and refusals for closed
//! addresses are tested next to `outbound/`.

use super::*;
use tokio::net::TcpStream;

mod low_latency {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn low_latency_sets_nodelay() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let tcp = TcpStream::connect(addr).await.unwrap();
        let (_accepted, _) = listener.accept().await.unwrap();
        assert!(!tcp.nodelay().unwrap(), "Nagle is on by default");

        disable_nagle(&tcp);

        assert!(tcp.nodelay().unwrap());
    }
}
