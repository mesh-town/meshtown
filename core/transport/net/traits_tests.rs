//! Object-safety check for the core traits: a non-object-safe trait will not compile.

use crate::transport::net::{Carrier, Connector, Discovery, PacketPath, PeerConn, Transport};

fn _p(_: &dyn PacketPath) {}
fn _c(_: &dyn Connector) {}
fn _k(_: &dyn Carrier) {}
fn _n(_: &dyn PeerConn) {}
fn _t(_: &dyn Transport) {}
fn _d(_: &dyn Discovery) {}

#[test]
fn traits_are_object_safe() {
    let _ = (_p, _c, _k, _n, _t, _d);
}
