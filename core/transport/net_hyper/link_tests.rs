//! Session counters and path/network information.

use super::*;
use crate::transport::net::link::PathKind;
use std::io::ErrorKind;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[test]
fn fresh_meter_reports_unknown_path() {
    let info = link_info(&LinkMeter::new());
    assert!(matches!(info.path, PathKind::Unknown));
    assert_eq!((info.sent_bytes, info.recv_bytes), (0, 0));
    assert_eq!((info.stream_frames, info.path_count), (0, 0));
    assert!(info.rtt.is_none() && info.remote.is_none());
}

#[tokio::test]
async fn metered_counts_both_directions() {
    let meter = Arc::new(LinkMeter::new());
    let (a, mut b) = duplex(4096);
    let mut a = Metered::new(a, meter.clone());
    a.write_all(&[1u8; 1000]).await.unwrap();
    b.write_all(&[2u8; 500]).await.unwrap();
    let mut got = [0u8; 500];
    a.read_exact(&mut got).await.unwrap();
    let info = link_info(&meter);
    assert_eq!((info.sent_bytes, info.recv_bytes), (1000, 500));
    assert!(info.stream_frames >= 1);
}

#[test]
fn established_meter_reports_direct_path_without_rtt() {
    let meter = LinkMeter::new();
    meter.mark_established();
    let info = link_info(&meter);
    assert!(matches!(info.path, PathKind::Direct));
    assert_eq!(info.path_count, 1);
    assert!(info.rtt.is_none() && info.remote.is_none());
    assert!(info.hint().is_none());
}

#[tokio::test]
async fn metered_fails_after_session_close() {
    let meter = Arc::new(LinkMeter::new());
    let (a, _b) = duplex(64);
    let mut a = Metered::new(a, meter.clone());
    meter.mark_established();
    meter.close();
    assert_eq!(link_info(&meter).path_count, 0);
    let w = a.write_all(b"x").await.unwrap_err();
    assert_eq!(w.kind(), ErrorKind::ConnectionAborted);
    let r = a.read(&mut [0u8; 1]).await.unwrap_err();
    assert_eq!(r.kind(), ErrorKind::ConnectionAborted);
}

#[test]
fn net_info_before_listen_has_no_net_seen() {
    let info = net_info(None);
    assert_eq!(info.net_seen, None);
}

#[test]
fn after_listen_ok_net_seen_true() {
    assert_eq!(net_info(Some(true)).net_seen, Some(true));
}

#[test]
fn after_listen_err_net_seen_false() {
    assert_eq!(net_info(Some(false)).net_seen, Some(false));
}
