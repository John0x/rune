use super::*;

fn ys(points: &[[f32; 2]]) -> Vec<f32> {
    points.iter().map(|p| p[1]).collect()
}

#[test]
fn monotone_curves_pass_through_every_point() {
    let pts = [[0.0, 0.0], [1.0, 1.0], [2.0, 1.0], [3.0, 0.0], [4.0, 1.0]];
    let c = monotone_x(&pts);
    assert_eq!(c.len(), 4);
    for (i, seg) in c.iter().enumerate() {
        assert_eq!((seg.from, seg.to), (pts[i], pts[i + 1]));
        assert!(seg.c1[0] > seg.from[0] && seg.c2[0] < seg.to[0], "controls stay between the points in x");
    }
}

#[test]
fn monotone_matches_the_d3_tangents() {
    let c = monotone_x(&[[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]);
    let third = 1.0 / 3.0;
    let close = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
    assert!(close(c[0].c1, [third, 0.5]), "{:?}", c[0]);
    assert!(close(c[0].c2, [2.0 * third, 1.0]), "a peak gets a flat tangent");
    assert!(close(c[1].c1, [1.0 + third, 1.0]));
    assert!(close(c[1].c2, [1.0 + 2.0 * third, 0.5]));
}

#[test]
fn monotone_data_stays_monotone_without_overshoot() {
    let rising = [[0.0, 0.0], [1.0, 0.1], [2.0, 5.0], [3.0, 5.1], [4.0, 9.0], [10.0, 9.5]];
    let line = flatten(&monotone_x(&rising), 0.01);
    assert!(line.windows(2).all(|w| w[1][1] >= w[0][1] - 1e-5 && w[1][0] >= w[0][0] - 1e-5));
    let plateau = [[0.0, 0.0], [1.0, 10.0], [2.0, 10.0], [3.0, 0.0]];
    let line = flatten(&monotone_x(&plateau), 0.01);
    assert!(ys(&line).iter().all(|y| (-1e-4..=10.0001).contains(y)), "never above the plateau");
    let flat = flatten(&monotone_x(&[[0.0, 3.0], [1.0, 3.0], [5.0, 3.0]]), 0.1);
    assert!(ys(&flat).iter().all(|y| *y == 3.0));
}

#[test]
fn monotone_edge_cases() {
    assert!(monotone_x(&[]).is_empty());
    assert!(monotone_x(&[[1.0, 1.0]]).is_empty());
    assert!(monotone_x(&[[1.0, 1.0], [1.0, 1.0]]).is_empty(), "duplicates collapse to one point");
    assert_eq!(monotone_x(&[[0.0, 0.0], [2.0, 2.0]]), [Cubic::line([0.0, 0.0], [2.0, 2.0])]);
    let vertical = monotone_x(&[[0.0, 0.0], [1.0, 0.0], [1.0, 5.0], [2.0, 5.0]]);
    assert!(vertical.iter().all(|c| [c.c1, c.c2].iter().all(|p| p[0].is_finite() && p[1].is_finite())));
}

#[test]
fn flattening_tracks_the_curve() {
    let c = Cubic { from: [0.0, 0.0], c1: [0.0, 100.0], c2: [100.0, 100.0], to: [100.0, 0.0] };
    let coarse = flatten(&[c], 2.0);
    let fine = flatten(&[c], 0.1);
    assert!(fine.len() > coarse.len());
    assert_eq!(fine[0], c.from);
    assert_eq!(*fine.last().unwrap(), c.to);
    // Every point of the true curve lies near the polyline.
    for k in 0..=200 {
        let p = c.at(k as f32 / 200.0);
        let d = fine.windows(2).map(|w| segment_distance(p, w[0], w[1])).fold(f32::MAX, f32::min);
        assert!(d <= 0.1 + 1e-3, "{d}");
    }
    assert_eq!(flatten(&[Cubic::line([0.0, 0.0], [5.0, 5.0])], 0.5), [[0.0, 0.0], [5.0, 5.0]]);
    assert!(flatten(&[], 0.5).is_empty());
    assert!(flatten(&[c], f32::NAN).len() > 2, "a bad tolerance falls back");
}

fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx * dx + dy * dy;
    let t = if len > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

#[test]
fn steps_turn_where_asked() {
    let pts = [[0.0, 0.0], [2.0, 4.0], [4.0, 1.0]];
    assert_eq!(step(&pts, StepPosition::After), [[0.0, 0.0], [2.0, 0.0], [2.0, 4.0], [4.0, 4.0], [4.0, 1.0]]);
    assert_eq!(step(&pts, StepPosition::Before), [[0.0, 0.0], [0.0, 4.0], [2.0, 4.0], [2.0, 1.0], [4.0, 1.0]]);
    assert_eq!(
        step(&pts, StepPosition::Middle),
        [[0.0, 0.0], [1.0, 0.0], [1.0, 4.0], [2.0, 4.0], [3.0, 4.0], [3.0, 1.0], [4.0, 1.0]]
    );
    assert_eq!(step(&pts[..1], StepPosition::After), [[0.0, 0.0]]);
    assert!(step(&[], StepPosition::Middle).is_empty());
}

#[test]
fn gaps_split_lines() {
    let pts = [[0.0, 0.0], [1.0, f32::NAN], [2.0, 2.0], [3.0, 3.0], [f32::INFINITY, 0.0], [5.0, 5.0]];
    let runs: Vec<&[[f32; 2]]> = split_gaps(&pts).collect();
    assert_eq!(runs, [&pts[0..1], &pts[2..4], &pts[5..6]]);
    assert_eq!(split_gaps(&[]).count(), 0);
    assert_eq!(split_gaps(&[[f32::NAN, 0.0]]).count(), 0);
}

#[test]
fn curve_kinds_give_polylines() {
    let pts = [[0.0, 0.0], [1.0, 2.0], [2.0, 1.0]];
    assert_eq!(Curve::Linear.polyline(&pts, 0.5), pts);
    assert_eq!(Curve::Step(StepPosition::After).polyline(&pts, 0.5), step(&pts, StepPosition::After));
    let smooth = Curve::MonotoneX.polyline(&pts, 0.01);
    assert!(smooth.len() > pts.len());
    assert_eq!((smooth[0], *smooth.last().unwrap()), (pts[0], pts[2]));
    assert_eq!(Curve::MonotoneX.polyline(&pts[..1], 0.5), [[0.0, 0.0]]);
    assert!(Curve::MonotoneX.polyline(&[], 0.5).is_empty());
    assert_eq!(Curve::default(), Curve::Linear);
}
