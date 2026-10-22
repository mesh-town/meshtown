//! Admission by service on a loopback cluster: a foreign key gets a `Refused` rejection, no session is created.

use super::gate::Admit;
use super::test_cluster::{cluster, Cluster};
use super::{HyperOpts, HyperTransport};
use crate::transport::net::addr::PeerAddr;
use crate::transport::net::contract::secret;
use crate::transport::net::error::NetError;
use crate::transport::net::service::{ServiceId, AUTH, EXIT, INVITE};
use crate::transport::net::Transport;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const WITHIN: Duration = Duration::from_secs(15);

struct Pair {
    _cluster: Cluster,
    a: Arc<HyperTransport>,
    b: Arc<HyperTransport>,
}

async fn pair() -> Pair {
    let cluster = cluster(3).await;
    let opts = || HyperOpts {
        bootstrap: Some(cluster.bootstrap()),
        ..Default::default()
    };
    let a = HyperTransport::bind(secret(1), opts())
        .await
        .expect("node A");
    let b = HyperTransport::bind(secret(2), opts())
        .await
        .expect("node B");
    b.serve(&[AUTH, INVITE, EXIT]);
    b.wait_listen_outcome().await.expect("listen B");
    Pair {
        _cluster: cluster,
        a,
        b,
    }
}

fn only_auth_invite() -> Option<Admit> {
    Some(Arc::new(|_, svc: &ServiceId| {
        *svc == AUTH || *svc == INVITE
    }))
}

async fn dial(p: &Pair, svc: ServiceId) -> Result<(), NetError> {
    let to = PeerAddr::new(p.b.me());
    let res = timeout(WITHIN, p.a.connect(&to, svc))
        .await
        .expect("connect hung");
    res.map(|_| ())
}

async fn b_gave(p: &Pair) -> ServiceId {
    timeout(WITHIN, p.b.accept())
        .await
        .expect("accept hung")
        .expect("queue closed")
        .0
}

#[tokio::test]
async fn stranger_is_refused_on_service() {
    let p = pair().await;
    p.b.set_admit(only_auth_invite());
    assert!(matches!(dial(&p, EXIT).await, Err(NetError::Refused)));
    assert!(
        timeout(Duration::from_millis(500), p.b.accept())
            .await
            .is_err(),
        "B must not hand over a session"
    );
}

#[tokio::test]
async fn stranger_reaches_auth_and_invite() {
    let p = pair().await;
    p.b.set_admit(only_auth_invite());
    dial(&p, AUTH).await.expect("AUTH");
    assert_eq!(b_gave(&p).await, AUTH);
    dial(&p, INVITE).await.expect("INVITE");
    assert_eq!(b_gave(&p).await, INVITE);
}

#[tokio::test]
async fn admit_is_read_live() {
    let p = pair().await;
    let open = Arc::new(AtomicBool::new(false));
    let flag = open.clone();
    p.b.set_admit(Some(Arc::new(move |_, _| flag.load(Ordering::SeqCst))));
    assert!(matches!(dial(&p, EXIT).await, Err(NetError::Refused)));
    open.store(true, Ordering::SeqCst);
    dial(&p, EXIT).await.expect("after the flag change");
    assert_eq!(b_gave(&p).await, EXIT);
}

#[tokio::test]
async fn no_admit_set_admits_all() {
    let p = pair().await;
    dial(&p, EXIT)
        .await
        .expect("admits everyone without a predicate");
    assert_eq!(b_gave(&p).await, EXIT);
}
