use super::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn first_four_rounds_are_fast() {
    let p = Pace::production();
    for round in 0..4 {
        assert_eq!(p.pause(round, 0, 12345), secs(15));
    }
}

#[test]
fn steady_pause_within_120_135() {
    let p = Pace::production();
    for spread in 0..2000u64 {
        let d = p.pause(4 + (spread % 7) as u32, 0, spread.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        assert!(d >= secs(120) && d <= secs(135), "{d:?}");
    }
}

#[test]
fn spread_is_deterministic() {
    let p = Pace::production();
    assert_eq!(p.pause(9, 0, 777), p.pause(9, 0, 777));
}

#[test]
fn error_pause_doubles_to_cap() {
    let p = Pace::production();
    let got: Vec<Duration> = (1..=5).map(|f| p.pause(10, f, 1)).collect();
    assert_eq!(got, vec![secs(10), secs(20), secs(40), secs(60), secs(60)]);
    assert_eq!(p.pause(10, 200, 1), secs(60));
}
