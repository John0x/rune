use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use crate::layout::{PieOptions, PieSort, pie};

#[test]
fn nearest_x_by_binary_search() {
    let x = [0.0, 10.0, 20.0, 30.0];
    let y = [1.0; 4];
    assert_eq!(nearest_x(&x, &y, 12.0), Some(1));
    assert_eq!(nearest_x(&x, &y, 16.0), Some(2));
    assert_eq!(nearest_x(&x, &y, -100.0), Some(0));
    assert_eq!(nearest_x(&x, &y, 100.0), Some(3));
    assert_eq!(nearest_x(&x, &y, 20.0), Some(2));
}

#[test]
fn nearest_x_ties_go_to_the_earlier_point() {
    let x = [0.0, 10.0, 20.0];
    assert_eq!(nearest_x(&x, &[1.0; 3], 15.0), Some(1));
    let dup = [0.0, 10.0, 10.0, 10.0, 20.0];
    assert_eq!(nearest_x(&dup, &[1.0; 5], 10.0), Some(1));
    assert_eq!(nearest_x(&dup, &[1.0; 5], 12.0), Some(1));
    assert_eq!(nearest_x(&dup, &[1.0; 5], 8.0), Some(1));
}

#[test]
fn nearest_x_skips_gaps() {
    let x = [0.0, 10.0, 20.0, 30.0, 40.0];
    let y = [1.0, f64::NAN, f64::NAN, 4.0, f64::NAN];
    assert_eq!(nearest_x(&x, &y, 12.0), Some(0));
    assert_eq!(nearest_x(&x, &y, 18.0), Some(3));
    assert_eq!(nearest_x(&x, &y, 45.0), Some(3));
    assert_eq!(nearest_x(&x, &[f64::NAN; 5], 10.0), None);
    assert_eq!(nearest_x(&[], &[], 10.0), None);
    assert_eq!(nearest_x(&x, &[1.0; 5], f64::NAN), None);
    assert_eq!(nearest_x(&x, &[1.0, 2.0], 35.0), Some(1), "columns of different lengths use the shorter");
}

#[test]
fn nearest_point_within_a_radius() {
    let x = [0.0, 1.0, 2.0, f64::NAN];
    let y = [0.0, 1.0, 0.0, 0.0];
    let to_px = |a: f64, b: f64| [(a * 100.0) as f32, (100.0 - b * 100.0) as f32];
    assert_eq!(nearest_point(&x, &y, to_px, [95.0, 5.0], 10.0), Some(1));
    assert_eq!(nearest_point(&x, &y, to_px, [50.0, 50.0], 10.0), None);
    assert_eq!(nearest_point(&x, &y, to_px, [100.0, 100.0], 100.0), Some(0), "a tie goes to the earlier point");
    assert_eq!(nearest_point(&[], &[], to_px, [0.0, 0.0], 10.0), None);
    assert_eq!(nearest_point(&x, &y, to_px, [0.0, 100.0], f32::NAN), None);
}

#[test]
fn bands_and_cells_under_the_pointer() {
    let cols = Band::new(24, (0.0, 240.0));
    let rows = Band::new(7, (0.0, 70.0)).padding_inner(0.1);
    assert_eq!(band_at(&cols, 125.0), Some(12));
    assert_eq!(cell_at(&cols, &rows, [125.0, 33.0]), Some((12, 3)));
    assert_eq!(cell_at(&cols, &rows, [125.0, 75.0]), None);
    assert_eq!(cell_at(&cols, &rows, [-1.0, 5.0]), None);
}

#[test]
fn pie_slices_under_the_pointer() {
    let slices = pie(&[1.0, 1.0, 1.0, 1.0], &PieOptions::default());
    let c = [100.0, 100.0];
    assert_eq!(slice_at(&slices, c, 0.0, 50.0, [110.0, 90.0]), Some(0));
    assert_eq!(slice_at(&slices, c, 0.0, 50.0, [130.0, 100.0]), Some(1), "three o'clock starts slice 1");
    assert_eq!(slice_at(&slices, c, 0.0, 50.0, [90.0, 110.0]), Some(2));
    assert_eq!(slice_at(&slices, c, 0.0, 50.0, [90.0, 90.0]), Some(3));
    assert_eq!(slice_at(&slices, c, 0.0, 50.0, [160.0, 100.0]), None, "outside the pie");
    assert_eq!(slice_at(&slices, c, 20.0, 50.0, [105.0, 95.0]), None, "in the donut's hole");
    assert_eq!(slice_at(&slices, c, 20.0, 50.0, [130.0, 110.0]), Some(1));
    assert_eq!(slice_at(&[], c, 0.0, 50.0, [110.0, 90.0]), None);
}

#[test]
fn pie_hits_report_the_value_index() {
    let sorted = pie(&[1.0, 3.0], &PieOptions { sort: PieSort::Descending, ..PieOptions::default() });
    assert_eq!(slice_at(&sorted, [0.0, 0.0], 0.0, 10.0, [5.0, -1.0]), Some(1), "the big slice comes first");
    let half = pie(&[1.0, 1.0], &PieOptions { start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, ..PieOptions::default() });
    assert_eq!(slice_at(&half, [0.0, 0.0], 0.0, 10.0, [-5.0, -1.0]), Some(0));
    assert_eq!(slice_at(&half, [0.0, 0.0], 0.0, 10.0, [0.0, 5.0]), None, "the empty half");
    let ccw = pie(&[1.0, 1.0], &PieOptions { start_angle: 0.0, end_angle: -TAU, ..PieOptions::default() });
    assert_eq!(
        slice_at(&ccw, [0.0, 0.0], 0.0, 10.0, [-5.0, -1.0]),
        Some(0),
        "counterclockwise, slice 0 is on the left"
    );
    assert_eq!(slice_at(&ccw, [0.0, 0.0], 0.0, 10.0, [5.0, -1.0]), Some(1));
}

#[test]
fn radar_axes_by_angle() {
    let c = [0.0, 0.0];
    assert_eq!(radar_axis_at(4, 0.0, c, [0.0, -10.0]), Some(0));
    assert_eq!(radar_axis_at(4, 0.0, c, [10.0, 1.0]), Some(1));
    assert_eq!(radar_axis_at(4, 0.0, c, [-1.0, 10.0]), Some(2));
    assert_eq!(radar_axis_at(4, 0.0, c, [-10.0, -2.0]), Some(3));
    assert_eq!(radar_axis_at(4, 0.0, c, [-1.0, -10.0]), Some(0), "just left of twelve wraps to axis 0");
    assert_eq!(radar_axis_at(3, PI, c, [0.0, 10.0]), Some(0));
    assert_eq!(radar_axis_at(4, 0.0, c, c), None);
    assert_eq!(radar_axis_at(0, 0.0, c, [1.0, 1.0]), None);
    assert_eq!(radar_axis_at(4, f64::NAN, c, [1.0, 1.0]), None);
}
