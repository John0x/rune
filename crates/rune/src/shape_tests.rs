use super::*;

fn area(points: &[[f32; 2]], tris: &[([usize; 3], u32)]) -> f32 {
    tris.iter()
        .map(|([a, b, c], _)| {
            let (a, b, c) = (points[*a], points[*b], points[*c]);
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
        })
        .sum()
}

#[test]
fn a_concave_polygon_triangulates_to_its_area_with_boundary_edges_marked() {
    // An L shape, clockwise in y-down screen space.
    let l = [
        [0.0, 0.0],
        [20.0, 0.0],
        [20.0, 10.0],
        [10.0, 10.0],
        [10.0, 30.0],
        [0.0, 30.0],
    ];
    let tris = triangulate(&l);
    assert_eq!(tris.len(), 4);
    assert!((area(&l, &tris) - 400.0).abs() < 1e-3);
    let boundary_edges: u32 = tris.iter().map(|(_, m)| m.count_ones()).sum();
    assert_eq!(
        boundary_edges, 6,
        "every polygon edge once, no interior edge"
    );
}

#[test]
fn a_waveform_envelope_triangulates_completely() {
    let top: Vec<[f32; 2]> = (0..200)
        .map(|i| [i as f32, 20.0 - ((i as f32) * 0.2).sin().abs() * 18.0 - 1.0])
        .collect();
    let mut env = top.clone();
    env.extend(top.iter().rev().map(|p| [p[0], 40.0 - p[1]]));
    let tris = triangulate(&env);
    assert_eq!(tris.len(), env.len() - 2);
    let exact: f32 = (0..env.len())
        .map(|i| {
            let (a, b) = (env[i], env[(i + 1) % env.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        .abs()
        * 0.5;
    assert!((area(&env, &tris) - exact).abs() < exact * 1e-3);
}

#[test]
fn curves_flatten_within_tolerance_and_end_on_their_points() {
    let p = Path::new()
        .move_to([0.0, 0.0])
        .cubic_to([0.0, 50.0], [100.0, 50.0], [100.0, 0.0])
        .close();
    let s = &p.subpaths[0];
    assert!(s.closed);
    assert_eq!(*s.points.last().unwrap(), [100.0, 0.0]);
    let mid = s.points[s.points.len() / 2];
    assert!((mid[1] - 37.5).abs() < 1.0, "{mid:?}");
}

fn strokes(s: &Shape) -> Vec<Vec<[f32; 2]>> {
    s.ops.iter().filter_map(|op| if let Op::Stroke { points, .. } = op { Some(points.clone()) } else { None }).collect()
}

fn fill_alpha(s: &Shape) -> Option<f32> {
    s.ops.iter().find_map(|op| if let Op::Fill { color, .. } = op { Some(color.a) } else { None })
}

#[test]
fn a_drawn_shape_traces_its_strokes_in_order_and_lands_its_head_last() {
    // A connector: an L-shaped line 100 points long, a second 20-point run, then its head.
    let s = Shape::new()
        .stroke(vec![[0.0, 0.0], [0.0, 60.0], [40.0, 60.0]], 1.0, Color::WHITE)
        .stroke(vec![[40.0, 70.0], [40.0, 90.0]], 1.0, Color::WHITE)
        .fill(vec![[36.0, 90.0], [44.0, 90.0], [40.0, 96.0]], Color::WHITE);
    assert_eq!(strokes(&s.drawn(0.0)), Vec::<Vec<[f32; 2]>>::new(), "nothing drawn yet");
    assert_eq!(fill_alpha(&s.drawn(0.0)), None);
    // 60 of 120 points: down the first leg, not yet round the corner.
    assert_eq!(strokes(&s.drawn(0.5)), vec![vec![[0.0, 0.0], [0.0, 60.0]]]);
    // 80 points: round the corner, halfway along the second leg.
    assert_eq!(strokes(&s.drawn(80.0 / 120.0)), vec![vec![[0.0, 0.0], [0.0, 60.0], [20.0, 60.0]]]);
    // The head fades in over the last 8 points of line.
    assert_eq!(fill_alpha(&s.drawn(108.0 / 120.0)), None);
    let a = fill_alpha(&s.drawn(116.0 / 120.0)).unwrap();
    assert!((a - 0.5).abs() < 1e-4, "{a}");
    assert_eq!(s.drawn(1.0), s, "drawn whole");
    assert_eq!(s.drawn(f32::NAN), s);
}

#[test]
fn a_closed_stroke_draws_on_round_to_its_start_and_fills_alone_fade() {
    let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let s = Shape::new().stroke_with(square, true, 1.0, LineCap::Round, Color::WHITE);
    assert_eq!(strokes(&s.drawn(0.875)), vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 5.0]]]);
    let dot = Shape::new().fill(vec![[0.0, 0.0], [4.0, 0.0], [2.0, 4.0]], Color::WHITE);
    assert_eq!(fill_alpha(&dot.drawn(0.25)), Some(0.25));
}

#[test]
fn a_trimmed_shape_keeps_the_stretch_between_start_and_end() {
    let s = Shape::new()
        .stroke(vec![[0.0, 0.0], [0.0, 60.0], [40.0, 60.0]], 1.0, Color::WHITE)
        .stroke(vec![[40.0, 70.0], [40.0, 90.0]], 1.0, Color::WHITE);
    // 30 to 105 of 120 points: round the corner, then 5 points down the second stroke.
    assert_eq!(
        strokes(&s.trimmed(0.25, 0.875)),
        vec![vec![[0.0, 30.0], [0.0, 60.0], [40.0, 60.0]], vec![[40.0, 70.0], [40.0, 75.0]]]
    );
    // A cut on a vertex keeps no zero-length piece there.
    assert_eq!(strokes(&s.trimmed(0.5, 0.875)), vec![vec![[0.0, 60.0], [40.0, 60.0]], vec![[40.0, 70.0], [40.0, 75.0]]]);
    assert!(strokes(&s.trimmed(0.6, 0.6)).is_empty(), "nothing between equal ends");
    assert!(strokes(&s.trimmed(0.7, 0.2)).is_empty());
    assert_eq!(s.trimmed(0.0, 1.0), s);
    assert_eq!(s.trimmed(f32::NAN, f32::NAN), s);
    assert_eq!(s.trimmed(0.0, 0.5), s.drawn(0.5));
}

#[test]
fn a_closed_stroke_trims_across_its_closing_edge() {
    let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let s = Shape::new().stroke_with(square, true, 1.0, LineCap::Round, Color::WHITE);
    assert_eq!(strokes(&s.trimmed(0.625, 1.0)), vec![vec![[5.0, 10.0], [0.0, 10.0], [0.0, 0.0]]]);
}

#[test]
fn empty_and_degenerate_ops_are_dropped() {
    let s = Shape::new()
        .stroke(vec![[0.0, 0.0]], 1.0, Color::WHITE)
        .fill(vec![[0.0, 0.0], [1.0, 1.0]], Color::WHITE);
    assert!(s.is_empty());
}

fn ring_area(points: &[[f32; 2]]) -> f32 {
    (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        .abs()
        * 0.5
}

fn fill(s: &Shape) -> (&[[f32; 2]], &[([usize; 3], u32)], Option<Gradient>) {
    s.ops
        .iter()
        .find_map(|op| if let Op::Fill { points, mesh, gradient, .. } = op { Some((points.as_slice(), &mesh.0[..], *gradient)) } else { None })
        .expect("a fill")
}

#[test]
fn an_area_triangulates_its_band_completely_with_only_its_outline_antialiased() {
    // A line over 500 points with uneven spacing, down to a baseline of 3 points.
    let upper: Vec<[f32; 2]> = (0..500).map(|i| [i as f32 * 0.7 + (i % 3) as f32 * 0.2, 40.0 - (i as f32 * 0.05).sin() * 30.0]).collect();
    let lower = vec![[0.0, 80.0], [150.0, 80.0], [350.0, 80.0]];
    let s = Shape::new().area(upper.clone(), lower.clone(), Color::WHITE);
    let (points, tris, _) = fill(&s);
    // The baseline's middle point is sampled on the line too; its ends share the line's.
    assert_eq!(points.len(), 2 * (upper.len() + 1));
    assert_eq!(tris.len(), points.len() - 2, "a band of n points is n - 2 triangles");
    assert!((area(points, tris) - ring_area(points)).abs() < ring_area(points) * 1e-4, "no triangle overlaps another");
    let turn = |[a, b, c]: [usize; 3]| {
        let (a, b, c) = (points[a], points[b], points[c]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    assert!(tris.iter().all(|(t, _)| turn(*t) >= 0.0), "all one way round");
    let outline: u32 = tris.iter().map(|(_, m)| m.count_ones()).sum();
    assert_eq!(outline as usize, points.len(), "every outline edge once, no inner edge");
}

#[test]
fn an_area_between_two_lines_matches_ear_clipping() {
    let upper: Vec<[f32; 2]> = (0..40).map(|i| [i as f32 * 5.0, 20.0 + (i % 4) as f32 * 3.0]).collect();
    let lower: Vec<[f32; 2]> = (0..25).map(|i| [i as f32 * 8.0 + 1.0, 60.0 + (i % 3) as f32 * 2.0]).collect();
    let s = Shape::new().area(upper.clone(), lower.clone(), Color::WHITE);
    let (points, tris, _) = fill(&s);
    let ear = triangulate(points);
    assert!((area(points, tris) - area(points, &ear)).abs() < 1e-2);
}

#[test]
fn a_short_area_falls_back_to_an_ordinary_fill() {
    let s = Shape::new().area(vec![[0.0, 0.0]], vec![[0.0, 10.0], [10.0, 10.0]], Color::WHITE);
    let (points, tris, _) = fill(&s);
    assert_eq!(points.len(), 3);
    assert_eq!(tris.len(), 1);
    assert!(Shape::new().area(vec![[0.0, 0.0]], vec![[1.0, 1.0]], Color::WHITE).is_empty());
}

#[test]
fn a_fill_keeps_its_triangles_through_clones_and_trims() {
    let s = Shape::new().fill(vec![[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]], Color::WHITE);
    let (_, tris, _) = fill(&s);
    assert_eq!(tris.len(), 2);
    let faded = s.drawn(0.5);
    let (_, kept, _) = fill(&faded);
    assert!(std::ptr::eq(tris, kept), "a trim shares the triangles, it doesn't triangulate again");
}

#[test]
fn a_gradient_fades_with_its_fill() {
    let g = Gradient::vertical(0.0, Color::WHITE, 10.0, Color { a: 0.5, ..Color::WHITE });
    let s = Shape::new().fill_gradient(vec![[0.0, 0.0], [4.0, 0.0], [2.0, 4.0]], g);
    assert_eq!(fill(&s).2, Some(g));
    let half = fill(&s.drawn(0.5)).2.expect("still a gradient");
    assert_eq!((half.start.a, half.end.a), (0.5, 0.25));
    assert_eq!(half.axis, GradientAxis::Vertical);
}

#[test]
fn a_dashed_stroke_is_its_dashes() {
    let s = Shape::new().stroke_dashed(vec![[0.0, 0.0], [25.0, 0.0]], 1.0, 4.0, 2.0, LineCap::Butt, Color::WHITE);
    let dashes = strokes(&s);
    // 0-4, 6-10, 12-16, 18-22, 24-25.
    assert_eq!(dashes.len(), 5);
    assert_eq!(dashes[1], vec![[6.0, 0.0], [10.0, 0.0]]);
    assert_eq!(dashes[4], vec![[24.0, 0.0], [25.0, 0.0]]);
    let solid = Shape::new().stroke_dashed(vec![[0.0, 0.0], [25.0, 0.0]], 1.0, 4.0, 0.0, LineCap::Butt, Color::WHITE);
    assert_eq!(strokes(&solid).len(), 1, "no gap is a solid line");
}

#[test]
fn a_dot_is_one_round_segment_of_its_diameter() {
    let s = Shape::new().dot([5.0, 6.0], 3.0, Color::WHITE);
    match &s.ops[..] {
        [Op::Stroke { points, width, cap, .. }] => {
            assert_eq!(points, &vec![[5.0, 6.0], [5.0, 6.0]]);
            assert_eq!((*width, *cap), (6.0, LineCap::Round));
        }
        ops => panic!("{ops:?}"),
    }
}



