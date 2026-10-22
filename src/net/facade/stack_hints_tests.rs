//! Tests of the running facade, continued: hints and addresses (`add_hints`, `set_relays`, invite).

use super::*;

#[tokio::test]
async fn add_hints_self_ignored() {
    let net = MemNet::new();
    let a = node(&net, 1, Opts::default());

    a.stack.add_hints(addr(1, &[1]));
    assert!(
        a.stack.0.mux.lookup(&pid(1)).is_none(),
        "we do not accumulate hints about ourselves"
    );
    assert_eq!(a.stack.snapshot().known_peer_count, 0);

    a.stack.add_hints(addr(2, &[1]));
    assert_eq!(
        a.stack.snapshot().known_peer_count,
        1,
        "control: a foreign hint is accepted"
    );
}

#[tokio::test]
async fn add_hints_forwarded_to_transports() {
    let net = MemNet::new();
    let a = node(&net, 1, Opts::default());

    a.stack.add_hints(addr(2, &[1]));

    within("the hint reached the transport", async {
        while a.spies[0].hints().is_empty() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert_eq!(a.spies[0].hints(), vec![addr(2, &[1])]);
}

#[tokio::test]
async fn own_address_found_by_discovery_not_forwarded() {
    let net = MemNet::new();
    let a = node(&net, 1, discovering(&net, &[]));
    let b = node(&net, 2, discovering(&net, &[]));
    within("publish A", a.stack.publish())
        .await
        .expect("publish A");
    within("publish B", b.stack.publish())
        .await
        .expect("publish B");

    within("the address of B reached the transport of A", async {
        while !a.spies[0].hints().iter().any(|h| h.id == pid(2)) {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    let own = a.spies[0].hints().iter().filter(|h| h.id == pid(1)).count();
    assert_eq!(own, 0, "the own address is not handed to the transport");
}

#[tokio::test]
async fn invite_and_publish_addrs() {
    let (x, y) = (MemNet::new(), MemNet::new());
    let writer = Script::new("mainline").publisher(ScriptReply::Ok).shared();
    let a = setup(
        &[&x, &y],
        1,
        Opts {
            sources: vec![writer.clone()],
            ..Opts::default()
        },
    );

    let invites = a.stack.invite_addrs();
    within("publish", a.stack.publish())
        .await
        .expect("publish succeeded");

    assert_eq!(invites.len(), 2, "one address per transport");
    assert!(invites
        .iter()
        .all(|one| one.id == pid(1) && one.hints.len() == 1));
    assert_ne!(
        invites[0].hints, invites[1].hints,
        "the networks differ — the hints differ"
    );
    let mut both: PeerAddr = invites[0].clone();
    both.merge(&invites[1]);
    assert_eq!(
        writer.published(),
        vec![both],
        "the publisher received the hints of both"
    );
}

#[tokio::test]
async fn set_relays_reaches_every_transport() {
    let (x, y) = (MemNet::new(), MemNet::new());
    let a = setup(&[&x, &y], 1, Opts::default());
    let relays = vec![RelayRef::Url("https://relay.example".into())];

    a.stack.set_relays(relays.clone());

    for spy in &a.spies {
        assert_eq!(
            spy.relays(),
            vec![relays.clone()],
            "relays — to every transport"
        );
    }
}
