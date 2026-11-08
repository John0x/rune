use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
}

#[test]
fn polar_runs_clockwise_from_twelve() {
    let c = [100.0, 100.0];
    assert!(near(polar(c, 0.0, 10.0), [100.0, 90.0]));
    assert!(near(polar(c, FRAC_PI_2, 10.0), [110.0, 100.0]));
    assert!(near(polar(c, PI, 10.0), [100.0, 110.0]));
    assert!(near(polar(c, -FRAC_PI_2, 10.0), [90.0, 100.0]));
}

#[test]
fn angle_at_inverts_polar() {
    let c = [3.0, -7.0];
    for a in [-3.0, -1.5, 0.0, 0.4, 1.5, 3.0] {
        assert!((angle_at(c, polar(c, a, 50.0)) - a).abs() < 1e-5, "{a}");
    }
    assert_eq!(angle_at(c, c), 0.0);
}
