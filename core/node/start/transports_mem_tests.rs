use super::*;
use crate::transport::net::{PeerAddr, AUTH};

fn secret() -> NodeSecret {
    crate::config::config_file::new_secret().unwrap()
}

/// A peer dialed by id alone, with no hint in the address.
#[tokio::test]
async fn dials_by_id_without_hint() {
    let net = MemNet::new();
    let (sa, sb) = (secret(), secret());
    let a = KeyedMem::new(&net, &sa);
    let b = KeyedMem::new(&net, &sb);
    b.serve(&[AUTH]);
    let to = PeerAddr::new(sb.peer_id());
    assert!(a.can_reach(&to), "a live slot must be reachable by id");
    let conn = a.connect(&to, AUTH).await;
    assert!(conn.is_ok(), "expected a connection by id");
}

/// A closed peer is unreachable, dialing gives an error.
#[tokio::test]
async fn gone_peer_unreachable() {
    let net = MemNet::new();
    let (sa, sb) = (secret(), secret());
    let a = KeyedMem::new(&net, &sa);
    let b = KeyedMem::new(&net, &sb);
    b.serve(&[AUTH]);
    b.close().await;
    let to = PeerAddr::new(sb.peer_id());
    assert!(!a.can_reach(&to), "a closed slot must not be reachable");
    assert!(a.connect(&to, AUTH).await.is_err());
}

/// A restarted peer (the same key) is dialed by id in the new slot.
#[tokio::test]
async fn restarted_peer_new_slot() {
    let net = MemNet::new();
    let (sa, sb) = (secret(), secret());
    let a = KeyedMem::new(&net, &sa);
    let b = KeyedMem::new(&net, &sb);
    b.close().await;
    let b2 = KeyedMem::new(&net, &sb);
    b2.serve(&[AUTH]);
    let to = PeerAddr::new(sb.peer_id());
    assert!(a.can_reach(&to));
    assert!(
        a.connect(&to, AUTH).await.is_ok(),
        "expected a connection to the new slot"
    );
}
