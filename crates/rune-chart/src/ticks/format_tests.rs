use super::*;

#[test]
fn precision_follows_the_step() {
    assert_eq!(precision(1.0), 0);
    assert_eq!(precision(50.0), 0);
    assert_eq!(precision(0.5), 1);
    assert_eq!(precision(0.2), 1);
    assert_eq!(precision(0.25), 2);
    assert_eq!(precision(0.01), 2);
    assert_eq!(precision(-0.1), 1);
    assert_eq!(precision(0.1 + 0.2), 1);
    assert_eq!(precision(0.0), 0);
    assert_eq!(precision(f64::NAN), 0);
    assert!((6..=15).contains(&precision(1.0 / 3.0)));
}

#[test]
fn numbers_take_the_steps_decimals() {
    assert_eq!(format_number(2.5, 0.5), "2.5");
    assert_eq!(format_number(2.0, 0.5), "2.0");
    assert_eq!(format_number(1200.0, 200.0), "1200");
    assert_eq!(format_number(0.1 + 0.2, 0.1), "0.3");
    assert_eq!(format_number(-0.04, 0.1), "0.0");
    assert_eq!(format_number(-0.0, 1.0), "0");
    assert_eq!(format_number(-1.5, 0.5), "-1.5");
}

#[test]
fn numbers_without_a_step_keep_up_to_six_decimals() {
    assert_eq!(format_number(1.23456789, 0.0), "1.234568");
    assert_eq!(format_number(2.5, f64::NAN), "2.5");
    assert_eq!(format_number(7.0, 0.0), "7");
    assert_eq!(format_number(f64::NAN, 1.0), "NaN");
    assert_eq!(format_number(f64::INFINITY, 1.0), "inf");
}

#[test]
fn compact_numbers_use_suffixes() {
    assert_eq!(format_compact(1500.0, 500.0), "1.5k");
    assert_eq!(format_compact(1000.0, 500.0), "1k");
    assert_eq!(format_compact(2e6, 1e6), "2M");
    assert_eq!(format_compact(3.4e6, 2e5), "3.4M");
    assert_eq!(format_compact(1.2e9, 1e8), "1.2G");
    assert_eq!(format_compact(5e12, 1e12), "5T");
    assert_eq!(format_compact(-2500.0, 500.0), "-2.5k");
    assert_eq!(format_compact(950.0, 50.0), "950");
    assert_eq!(format_compact(0.25, 0.05), "0.25");
    assert_eq!(format_compact(0.0, 1000.0), "0");
    assert_eq!(format_compact(-0.0001, 1.0), "0");
}

#[test]
fn compact_rounds_to_the_step_before_picking_a_suffix() {
    assert_eq!(format_compact(999_950.0, 100.0), "1M");
    assert_eq!(format_compact(999.6, 1.0), "1k");
    assert_eq!(format_compact(1234.0, 0.0), "1.234k");
    assert_eq!(format_compact(f64::NEG_INFINITY, 1.0), "-inf");
}
