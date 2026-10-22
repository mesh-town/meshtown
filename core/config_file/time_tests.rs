use super::*;

#[test]
fn utc_known_dates() {
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    // 2000 is a leap year (divisible by 400).
    assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    // 2100 is not a leap year: a century year not divisible by 400.
    assert_eq!(iso_utc(4_107_542_400), "2100-03-01T00:00:00Z");
    assert_eq!(iso_utc(1_790_278_867), "2026-09-24T19:41:07Z");
    assert_eq!(stamp_utc(1_790_278_867), "20260924-194107");
}
