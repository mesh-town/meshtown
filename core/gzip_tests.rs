//! Tests of unpacking the embedded gzip bodies.

use super::inflate_gzip;

fn index_gz() -> Vec<u8> {
    include_bytes!(concat!(env!("OUT_DIR"), "/ui/index.html.gz")).to_vec()
}

#[test]
fn inflates_the_index_entry() {
    // given
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web_gui/ui/index.html");
    // when
    let out = inflate_gzip(&index_gz()).expect("inflate");
    // then
    assert_eq!(out, std::fs::read(path).unwrap());
}

#[test]
fn rejects_bad_magic_short_input_and_flags() {
    // given
    let mut bad_magic = index_gz();
    bad_magic[1] = 0x8c;
    let mut flagged = index_gz();
    flagged[3] = 0x08;
    let inputs = [vec![], index_gz()[..17].to_vec(), bad_magic, flagged];
    // then
    for input in inputs {
        assert!(inflate_gzip(&input).is_err(), "{} bytes", input.len());
    }
}

#[test]
fn rejects_isize_mismatch() {
    // given
    let mut gz = index_gz();
    *gz.last_mut().unwrap() ^= 0x01;
    // then
    assert!(inflate_gzip(&gz).is_err());
}
