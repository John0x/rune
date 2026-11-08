use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

#[test]
fn axes_spread_evenly() {
    assert_eq!(radar_angles(4, 0.0), [0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2]);
    assert_eq!(radar_angles(2, 1.0), [1.0, 1.0 + PI]);
    assert!(radar_angles(0, 0.0).is_empty());
}

#[test]
fn values_sit_on_their_axes() {
    let c = [100.0, 100.0];
    let r = Linear::new((0.0, 10.0), (0.0, 50.0));
    let p = radar_points(c, &[10.0, 5.0, 0.0, 2.0], &r, 0.0);
    assert!(near(p[0], [100.0, 50.0]));
    assert!(near(p[1], [125.0, 100.0]));
    assert!(near(p[2], c));
    assert!(near(p[3], [90.0, 100.0]));
}

#[test]
fn odd_values_sit_at_the_center() {
    let c = [0.0, 0.0];
    let r = Linear::new((0.0, 10.0), (0.0, 50.0));
    let p = radar_points(c, &[f64::NAN, -5.0, f64::INFINITY], &r, 0.0);
    assert!(p.iter().all(|&q| near(q, c)));
    assert!(radar_points(c, &[], &r, 0.0).is_empty());
}

#[test]
fn rings_pass_through_every_axis() {
    let ring = radar_ring([0.0, 0.0], 6, 20.0, 0.0);
    assert_eq!(ring.len(), 6);
    assert!(ring.iter().all(|p| (p[0].hypot(p[1]) - 20.0).abs() < 1e-3));
    assert!(near(ring[0], [0.0, -20.0]));
}
