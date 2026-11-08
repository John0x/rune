use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

#[test]
fn nice_max_rounds_up_to_nice_rings() {
    assert_eq!(nice_max(9.0, 4), 10.0);
    assert_eq!(nice_max(10.0, 4), 10.0);
    assert_eq!(nice_max(87.0, 4), 100.0);
    assert_eq!(nice_max(130.0, 5), 150.0);
    assert_eq!(nice_max(0.7, 4), 0.8);
    assert_eq!(nice_max(3.0, 3), 3.0);
}

#[test]
fn nice_max_without_data_is_one_per_ring() {
    assert_eq!(nice_max(0.0, 4), 4.0);
    assert_eq!(nice_max(-3.0, 4), 4.0);
    assert_eq!(nice_max(f64::NAN, 2), 2.0);
    assert_eq!(nice_max(5.0, 0), 5.0);
}

#[test]
fn axis_labels_sit_outside_the_ring() {
    let (w, lh) = (40.0, 14.0);
    // Top: centred above.
    assert_eq!(axis_label_origin([100.0, 20.0], 0.0, w, lh), [80.0, 6.0]);
    // Right: starts at the anchor, centred on it.
    assert_eq!(axis_label_origin([180.0, 100.0], FRAC_PI_2, w, lh), [180.0, 93.0]);
    // Bottom: centred below.
    assert_eq!(axis_label_origin([100.0, 180.0], PI, w, lh), [80.0, 180.0]);
    // Left: ends at the anchor.
    assert_eq!(axis_label_origin([20.0, 100.0], 3.0 * FRAC_PI_2, w, lh), [-20.0, 93.0]);
}
