use super::*;
use crate::config::config_file::NetSlotId;
use crate::ports_sharing::record::PORT_RECORD_VERSION;
use crate::ports_sharing::{Audience, PortRecord};

/// A valid, distinct id per `n`: the last byte is searched until the bytes are a curve point.
fn pid(n: u8) -> PeerId {
    let mut bytes = [0u8; 32];
    bytes[0] = n;
    (0..=255u8)
        .find_map(|tail| {
            bytes[31] = tail;
            PeerId::from_bytes(&bytes).ok()
        })
        .expect("some tail makes a valid id")
}

fn device(n: u8, name: &str) -> Device {
    Device {
        net: NetSlotId::main(),
        peer: pid(n),
        name: name.to_string(),
        label: name.to_lowercase(),
        online: true,
    }
}

fn view(author: u8, port: u16, name: &str, audience: Audience) -> PortView {
    PortView {
        author: pid(author),
        port,
        proto: Proto::Tcp,
        record: Some(PortRecord {
            v: PORT_RECORD_VERSION,
            name: name.to_string(),
            audience,
        }),
    }
}

fn run(devices: &[Device], views: &[PortView]) -> Vec<(String, SharedDevice)> {
    let issued = addresses(devices);
    shared_devices(devices, &issued, views, &pid(9))
}

#[test]
fn connected_lists_only_ports_open_to_me() {
    let devices = [device(1, "Box")];
    let views = [
        view(1, 80, "web", Audience::All),
        view(1, 81, "closed", Audience::Nodes(vec![pid(5)])),
        view(1, 82, "mine", Audience::Nodes(vec![pid(9)])),
    ];

    let found = run(&devices, &views);

    let ports: Vec<u16> = found[0].1.open.iter().map(|p| p.port).collect();
    assert_eq!(ports, vec![80, 82]);
}

#[test]
fn connected_skips_devices_without_open_ports() {
    let devices = [device(1, "Box"), device(2, "Cam")];
    let views = [view(2, 80, "web", Audience::Nodes(vec![pid(5)]))];

    let found = run(&devices, &views);

    assert!(found.is_empty());
}

#[test]
fn connected_sorts_by_address_then_port() {
    let devices = [device(1, "Zed"), device(2, "Alpha")];
    let views = [
        view(1, 8080, "", Audience::All),
        view(2, 9000, "", Audience::All),
        view(2, 81, "", Audience::All),
    ];

    let found = run(&devices, &views);

    let order: Vec<(String, Vec<u16>)> = found
        .into_iter()
        .map(|(a, d)| (a, d.open.iter().map(|p| p.port).collect()))
        .collect();
    assert_eq!(
        order,
        vec![
            ("alpha".to_string(), vec![81, 9000]),
            ("zed".to_string(), vec![8080])
        ]
    );
}

#[test]
fn connected_skips_udp_ports() {
    let devices = [device(1, "Box")];
    let mut udp = view(1, 53, "dns", Audience::All);
    udp.proto = Proto::Udp;

    assert!(run(&devices, &[udp]).is_empty());
}
