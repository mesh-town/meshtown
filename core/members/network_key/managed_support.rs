//! Shared fixtures of the Managed network key tests: the golden strings and the small helpers.

use super::*;
use crate::members::network_key::parse_user_input;
use crate::transport::net::NodeSecret;
use data_encoding::{BASE32_NOPAD, HEXLOWER};

/// Random part shared by every golden string.
pub(super) const TAIL: &str = "abacus-abdomen-0123456789abcdefghjkm";
/// Base32 of the creator's node id, creator = node secret `[7; 32]`.
pub(super) const ROOT52: &str = "5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";
/// The creator's node id itself.
pub(super) const ROOT_HEX: &str =
    "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";
/// What `TAIL` derives to: the base of the Managed key.
pub(super) const BASE_HEX: &str =
    "daf2008f0eb0190eb5be32c55e9ad92084b54d4d0b57b5d2786c67933c4efee6";
/// Key bytes and id of the Managed network made of `TAIL` and the creator `[7; 32]`.
pub(super) const KEY_HEX: &str = "356cc856e9ee8cc150202a9e18dbab1a0d6f950ec6b075e76e03602ee4ec8671";
pub(super) const KEY_ID: &str = "9d815c4a";
/// The same base with the creator `[8; 32]`.
pub(super) const OTHER_KEY_HEX: &str =
    "37a95150ca9373b0ab8a87f19568ec158ad852758c0d62a7b929590e978e83a9";
pub(super) const OTHER_KEY_ID: &str = "1be1fc57";

/// Words form with a label.
pub(super) const G1: &str = "work--abacus-abdomen-0123456789abcdefghjkm-5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";
/// Raw form with a label.
pub(super) const G2: &str = "work--mesh2-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73toustmmprjyuqkx32va6ytf3c7tfkho2xl5pt3sjbb52tjcrdnela";
/// Raw form without a label.
pub(super) const G3: &str = "mesh2-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73toustmmprjyuqkx32va6ytf3c7tfkho2xl5pt3sjbb52tjcrdnela";
/// Words form without a label.
pub(super) const G4: &str =
    "abacus-abdomen-0123456789abcdefghjkm-5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";

pub(super) fn parse(text: &str) -> NetworkKeyInput {
    parse_user_input(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// The golden Managed network: creator `[7; 32]`, base derived from `TAIL`.
pub(super) fn assert_managed(input: &NetworkKeyInput, label: Option<&str>, from_words: bool) {
    assert_eq!(input.label.as_deref(), label, "label");
    assert_eq!(input.from_words, from_words, "from_words");
    assert_eq!(input.root, root_of(7), "root");
    assert_eq!(HEXLOWER.encode(&input.base), BASE_HEX, "base");
    assert_eq!(HEXLOWER.encode(&input.key.to_bytes()), KEY_HEX, "key bytes");
    assert_eq!(input.key.subkeys().network_id_hex(), KEY_ID, "network id");
}

pub(super) fn root_of(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

pub(super) fn hex32(text: &str) -> [u8; 32] {
    HEXLOWER
        .decode(text.as_bytes())
        .unwrap()
        .try_into()
        .unwrap()
}

pub(super) fn b32(bytes: &[u8]) -> String {
    BASE32_NOPAD.encode(bytes).to_ascii_lowercase()
}

/// The first byte pattern `[i; 32]` that is not a curve point.
pub(super) fn off_curve_bytes() -> [u8; 32] {
    (0..=255u8)
        .map(|i| [i; 32])
        .find(|bytes| PeerId::from_bytes(bytes).is_err())
        .expect("some repeated byte pattern is off the curve")
}

/// A freshly made Managed network: its text, key, creator and the secret of the creator.
pub(crate) struct TestNet {
    /// The raw form with the label: `label--mesh2-…`.
    pub text: String,
    pub key: NetworkKey,
    pub root: PeerId,
    /// What the key is derived from; the secret an invite redemption answers with.
    pub base: [u8; 32],
}

/// Makes a new Managed network under the label; every call gives a different one.
pub(crate) fn test_net(label: &str) -> TestNet {
    let root_secret = NodeSecret::generate();
    let root = root_secret.peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    TestNet {
        text: managed_key_with_label(Some(label), &base, &root),
        key: managed_key(&base, &root),
        root,
        base,
    }
}
