use super::*;

fn v4(port: u16) -> SocketAddr {
    SocketAddr::from(([10, 1, 2, 3], port))
}

#[test]
fn data_frame_round_trips_for_ipv4() {
    let frame = Frame::data(7, v4(53), b"hello".to_vec());
    let bytes = encode(&frame).expect("fits");
    assert_eq!(&bytes[..2], &((bytes.len() - 2) as u16).to_be_bytes());
    assert_eq!(parse_body(&bytes[2..]).expect("parses"), frame);
}

#[test]
fn data_frame_round_trips_for_ipv6() {
    let addr = SocketAddr::from((Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0x1111), 443));
    let frame = Frame::data(9, addr, vec![1, 2, 3]);
    let bytes = encode(&frame).expect("fits");
    assert_eq!(parse_body(&bytes[2..]).expect("parses"), frame);
}

#[test]
fn domain_target_is_parsed_and_lowercased() {
    let mut body = vec![FLAG_DATA, 0, 1, ATYP_DOMAIN, 4];
    body.extend_from_slice(b"EchO");
    body.extend_from_slice(&5000u16.to_be_bytes());
    body.extend_from_slice(b"x");
    let frame = parse_body(&body).expect("parses");
    assert_eq!(frame.target, Some(Target::Domain("echo".into(), 5000)));
    assert_eq!(frame.data, b"x");
}

#[test]
fn keepalive_and_close_have_no_address() {
    let bytes = encode(&Frame::keepalive(3)).expect("fits");
    assert_eq!(bytes, [0, 3, FLAG_KEEPALIVE, 0, 3]);
    let close = parse_body(&[FLAG_CLOSE, 0, 9]).expect("parses");
    assert_eq!(
        (close.flags, close.conn_id, close.target),
        (FLAG_CLOSE, 9, None)
    );
}

#[test]
fn unknown_flag_bits_are_kept_and_not_an_error() {
    let frame = parse_body(&[0x40 | FLAG_KEEPALIVE, 0, 1]).expect("parses");
    assert_eq!(frame.flags, 0x41);
}

#[test]
fn cut_frames_are_errors() {
    assert!(parse_body(&[FLAG_DATA, 0]).is_err());
    assert!(parse_body(&[FLAG_DATA, 0, 1]).is_err());
    assert!(parse_body(&[FLAG_DATA, 0, 1, ATYP_IPV4, 1, 2]).is_err());
    assert!(parse_body(&[FLAG_DATA, 0, 1, ATYP_DOMAIN, 9, b'a']).is_err());
    assert!(parse_body(&[FLAG_DATA, 0, 1, 9, 1, 2, 3, 4, 0, 1]).is_err());
}

#[test]
fn oversized_frame_is_not_built() {
    assert!(encode(&Frame::data(1, v4(1), vec![0; 70_000])).is_none());
}

#[tokio::test]
async fn read_frame_reports_cut_stream() {
    let mut whole: &[u8] = &[0, 3, FLAG_CLOSE, 0, 4];
    assert_eq!(read_frame(&mut whole).await.expect("frame").conn_id, 4);
    let mut cut: &[u8] = &[0, 9, FLAG_CLOSE];
    assert!(read_frame(&mut cut).await.is_err());
}

#[test]
fn error_frame_has_the_error_flag_and_no_address() {
    assert_eq!(
        encode(&Frame::error(9)).expect("fits"),
        vec![0, 3, FLAG_ERR, 0, 9]
    );
}
