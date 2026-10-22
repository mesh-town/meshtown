use super::*;

fn bits(x: f64) -> u64 {
    x.to_bits()
}

#[test]
fn maximum_picks_the_larger() {
    assert_eq!(maximum_num(1.0, 2.0), 2.0);
    assert_eq!(maximum_num(2.0, 1.0), 2.0);
    assert_eq!(maximum_num(-3.0, -4.0), -3.0);
}

#[test]
fn minimum_picks_the_smaller() {
    assert_eq!(minimum_num(1.0, 2.0), 1.0);
    assert_eq!(minimum_num(2.0, 1.0), 1.0);
    assert_eq!(minimum_num(-3.0, -4.0), -4.0);
}

#[test]
fn a_nan_argument_yields_the_other() {
    assert_eq!(maximum_num(f64::NAN, 5.0), 5.0);
    assert_eq!(maximum_num(5.0, f64::NAN), 5.0);
    assert_eq!(minimum_num(f64::NAN, 5.0), 5.0);
    assert_eq!(minimum_num(5.0, f64::NAN), 5.0);
    assert!(maximum_num(f64::NAN, f64::NAN).is_nan());
    assert!(minimum_num(f64::NAN, f64::NAN).is_nan());
}

#[test]
fn negative_zero_is_below_positive_zero() {
    assert_eq!(bits(maximum_num(-0.0, 0.0)), bits(0.0));
    assert_eq!(bits(maximum_num(0.0, -0.0)), bits(0.0));
    assert_eq!(bits(minimum_num(-0.0, 0.0)), bits(-0.0));
    assert_eq!(bits(minimum_num(0.0, -0.0)), bits(-0.0));
}

#[test]
fn infinities_are_ordered() {
    assert_eq!(maximum_num(f64::NEG_INFINITY, f64::INFINITY), f64::INFINITY);
    assert_eq!(minimum_num(f64::NEG_INFINITY, f64::INFINITY), f64::NEG_INFINITY);
}

#[test]
fn f32_versions_follow_the_same_rules() {
    assert_eq!(maximum_numf(1.0, 2.0), 2.0);
    assert_eq!(minimum_numf(1.0, 2.0), 1.0);
    assert_eq!(maximum_numf(f32::NAN, 5.0), 5.0);
    assert_eq!(minimum_numf(5.0, f32::NAN), 5.0);
    assert_eq!(maximum_numf(-0.0, 0.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(minimum_numf(0.0, -0.0).to_bits(), (-0.0f32).to_bits());
}
