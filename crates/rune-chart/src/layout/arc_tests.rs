use std::f64::consts::PI;

use super::*;
use crate::layout::{PieOptions, angle_at, pie};

const C: [f32; 2] = [100.0, 100.0];

fn radius(p: [f32; 2]) -> f32 {
    (p[0] - C[0]).hypot(p[1] - C[1])
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
fn full_circles_and_rings() {
    let circle = arc_polygon(C, 0.0, 50.0, 0.0, TAU, 0.0, 0.5);
    assert!(circle.iter().all(|&p| (radius(p) - 50.0).abs() < 1e-3));
    assert!(dist(circle[0], *circle.last().unwrap()) < 1e-3, "closed");
    let ring = arc_polygon(C, 30.0, 50.0, 0.0, TAU, 0.0, 0.5);
    let (outer, inner): (Vec<_>, Vec<_>) = ring.iter().partition(|&&p| radius(p) > 40.0);
    assert_eq!(outer.len() + inner.len(), ring.len());
    assert!(inner.iter().all(|&p| (radius(p) - 30.0).abs() < 1e-3));
    assert!(!inner.is_empty());
}

#[test]
fn wedges_end_at_the_center_and_segments_at_the_inner_arc() {
    let wedge = arc_polygon(C, 0.0, 50.0, 0.0, FRAC_PI_2, 0.0, 0.5);
    assert_eq!(*wedge.last().unwrap(), C);
    assert!(dist(wedge[0], [100.0, 50.0]) < 1e-3);
    let seg = arc_polygon(C, 50.0, 30.0, 0.0, FRAC_PI_2, 0.0, 0.5);
    assert!(dist(seg[0], [100.0, 50.0]) < 1e-3, "radii in either order");
    assert!(dist(*seg.last().unwrap(), [100.0, 70.0]) < 1e-3);
}

#[test]
fn flattening_stays_within_the_tolerance() {
    for tolerance in [0.1f32, 0.5, 2.0] {
        let pts = arc_polygon(C, 0.0, 200.0, 0.0, PI, 0.0, tolerance);
        let arc = &pts[..pts.len() - 1];
        for w in arc.windows(2) {
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!(200.0 - radius(mid) <= tolerance + 1e-3, "{tolerance}");
        }
    }
    let coarse = arc_polygon(C, 0.0, 200.0, 0.0, PI, 0.0, 2.0).len();
    let fine = arc_polygon(C, 0.0, 200.0, 0.0, PI, 0.0, 0.1).len();
    assert!(fine > coarse);
    assert!(arc_polygon(C, 0.0, 200.0, 0.0, PI, 0.0, 0.0).len() > 2, "a zero tolerance falls back");
}

#[test]
fn padding_leaves_gaps_of_even_width() {
    let slices = pie(&[1.0, 1.0, 1.0], &PieOptions { pad_angle: 0.05, ..PieOptions::default() });
    let (a, b) = (slices[0], slices[1]);
    let pa = arc_polygon(C, 40.0, 100.0, a.start, a.end, a.pad, 0.25);
    let pb = arc_polygon(C, 40.0, 100.0, b.start, b.end, b.pad, 0.25);
    let outer_end = pa.iter().copied().rfind(|&p| radius(p) > 99.0).unwrap();
    let outer_start = pb.iter().copied().find(|&p| radius(p) > 99.0).unwrap();
    let inner_start = pa.iter().copied().find(|&p| radius(p) < 41.0).unwrap();
    let inner_end = pb.iter().copied().rfind(|&p| radius(p) < 41.0).unwrap();
    let (gap_out, gap_in) = (dist(outer_end, outer_start), dist(inner_start, inner_end));
    assert!((gap_out - gap_in).abs() < 0.2, "{gap_out} vs {gap_in}");
    let rp = (40.0f64.powi(2) + 100.0f64.powi(2)).sqrt();
    assert!((f64::from(gap_out) - rp * 0.05).abs() < 0.2);
}

#[test]
fn padding_wider_than_a_slice_collapses_it() {
    let tiny = arc_polygon(C, 40.0, 100.0, 0.0, 0.01, 0.5, 0.25);
    assert!(!tiny.is_empty());
    let mid = 0.005;
    assert!(tiny.iter().all(|&p| (angle_at(C, p) - mid).abs() < 1e-3));
}

#[test]
fn odd_inputs_give_nothing() {
    assert!(arc_polygon(C, 0.0, 0.0, 0.0, 1.0, 0.0, 0.5).is_empty());
    assert!(arc_polygon(C, 0.0, -5.0, 0.0, 1.0, 0.0, 0.5).is_empty());
    assert!(arc_polygon(C, 0.0, f32::NAN, 0.0, 1.0, 0.0, 0.5).is_empty());
    assert!(arc_polygon(C, 0.0, 10.0, f64::NAN, 1.0, 0.0, 0.5).is_empty());
    assert!(!arc_polygon(C, 0.0, 10.0, 0.0, 1.0, f64::NAN, 0.5).is_empty());
}

#[test]
fn centroids_and_leader_lines() {
    let c = arc_centroid(C, 0.0, 100.0, 0.0, FRAC_PI_2);
    let d = 50.0 * std::f32::consts::FRAC_1_SQRT_2;
    assert!(dist(c, [100.0 + d, 100.0 - d]) < 1e-3);
    let right = leader_line(C, 50.0, 0.0, FRAC_PI_2, 10.0, 20.0);
    assert_eq!(right.side, LabelSide::Right);
    assert!((radius(right.points[0]) - 50.0).abs() < 1e-3);
    assert!((radius(right.points[1]) - 60.0).abs() < 1e-3);
    assert_eq!(right.points[2], [right.points[1][0] + 20.0, right.points[1][1]]);
    let left = leader_line(C, 50.0, PI, 2.0 * PI, 10.0, 20.0);
    assert_eq!(left.side, LabelSide::Left);
    assert_eq!(left.points[2][0], left.points[1][0] - 20.0);
}
