//! Punycode (RFC 3492) encoder for one domain label.

const BASE: u32 = 36;
const T_MIN: u32 = 1;
const T_MAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;

/// Encodes a label without the `xn--` prefix; `None` on arithmetic overflow.
pub(crate) fn encode_label(label: &str) -> Option<String> {
    let input: Vec<u32> = label.chars().map(u32::from).collect();
    let mut out: String = label.chars().filter(char::is_ascii).collect();
    let basic = u32::try_from(out.len()).ok()?;
    if basic > 0 {
        out.push('-');
    }
    encode_rest(&input, basic, &mut out)?;
    Some(out)
}

fn encode_rest(input: &[u32], basic: u32, out: &mut String) -> Option<()> {
    let total = u32::try_from(input.len()).ok()?;
    let (mut n, mut delta, mut bias, mut handled) = (INITIAL_N, 0u32, INITIAL_BIAS, basic);
    while handled < total {
        let next = input.iter().copied().filter(|&c| c >= n).min()?;
        delta = delta.checked_add((next - n).checked_mul(handled + 1)?)?;
        n = next;
        for &c in input {
            let (d, hit) = step(c, n, delta)?;
            delta = d;
            if hit {
                push_delta(out, delta, bias);
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(())
}

/// For one code point: the new delta and whether it is exactly the one being emitted.
fn step(c: u32, n: u32, delta: u32) -> Option<(u32, bool)> {
    match c.cmp(&n) {
        std::cmp::Ordering::Less => Some((delta.checked_add(1)?, false)),
        std::cmp::Ordering::Equal => Some((delta, true)),
        std::cmp::Ordering::Greater => Some((delta, false)),
    }
}

fn threshold(k: u32, bias: u32) -> u32 {
    if k <= bias {
        return T_MIN;
    }
    if k >= bias + T_MAX {
        return T_MAX;
    }
    k - bias
}

fn push_delta(out: &mut String, delta: u32, bias: u32) {
    let (mut q, mut k) = (delta, BASE);
    loop {
        let t = threshold(k, bias);
        if q < t {
            break;
        }
        out.push(digit(t + (q - t) % (BASE - t)));
        q = (q - t) / (BASE - t);
        k += BASE;
    }
    out.push(digit(q));
}

fn adapt(delta: u32, points: u32, first: bool) -> u32 {
    let mut d = if first { delta / DAMP } else { delta / 2 };
    d += d / points;
    let mut k = 0;
    while d > (BASE - T_MIN) * T_MAX / 2 {
        d /= BASE - T_MIN;
        k += BASE;
    }
    k + (BASE - T_MIN + 1) * d / (d + SKEW)
}

fn digit(d: u32) -> char {
    match d {
        0..=25 => char::from(b'a' + d as u8),
        _ => char::from(b'0' + (d - 26) as u8),
    }
}

#[cfg(test)]
#[path = "punycode_tests.rs"]
mod tests;
