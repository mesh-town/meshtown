//! Deterministic pseudo-random number generator of the demo node (xorshift64).

pub(crate) fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

pub(crate) fn gen_hex8(state: &mut u64) -> String {
    format!("{:08x}", (xorshift64(state) & 0xffff_ffff) as u32)
}
