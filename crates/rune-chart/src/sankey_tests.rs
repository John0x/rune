use super::*;

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

#[test]
fn a_transform_inverts() {
    let t = Transform { scale: 2.5, offset: [10.0, -20.0] };
    let p = [33.0, 47.0];
    assert!(near(t.invert(t.apply(p)), p));
    assert!(near(Transform::default().apply(p), p));
}

#[test]
fn zoom_keeps_the_point_under_the_pointer() {
    let t = Transform { scale: 1.5, offset: [20.0, 5.0] };
    let at = [120.0, 80.0];
    let under = t.invert(at);
    let z = t.zoom_at(at, 2.0);
    assert_eq!(z.scale, 3.0);
    assert!(near(z.apply(under), at));
}

#[test]
fn zoom_stays_within_limits() {
    let t = Transform::default();
    assert_eq!(t.zoom_at([0.0, 0.0], 100.0).scale, MAX_SCALE);
    assert_eq!(t.zoom_at([0.0, 0.0], 0.01).scale, MIN_SCALE);
    assert_eq!(t.zoom_at([5.0, 5.0], f32::NAN), t);
    assert_eq!(t.zoom_at([5.0, 5.0], 0.0), t);
}

#[test]
fn pan_moves_and_clamp_keeps_some_in_view() {
    let t = Transform::default().pan([30.0, -10.0]);
    assert_eq!(t.offset, [30.0, -10.0]);
    assert_eq!(t.clamp([400.0, 300.0]), t);
    // Dragged far off to the right and down: the diagram's left and top edges stay `KEEP` in.
    let far = Transform::default().pan([1000.0, 1000.0]).clamp([400.0, 300.0]);
    assert_eq!(far.offset, [400.0 - KEEP, 300.0 - KEEP]);
    // Far off to the left at 2x: its right edge stays `KEEP` in.
    let far = Transform { scale: 2.0, offset: [-5000.0, 0.0] }.clamp([400.0, 300.0]);
    assert_eq!(far.offset[0], KEEP - 800.0);
}

#[test]
fn points_in_polygons() {
    let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert!(point_in_polygon(&square, [5.0, 5.0]));
    assert!(!point_in_polygon(&square, [15.0, 5.0]));
    assert!(!point_in_polygon(&square, [5.0, -1.0]));
    assert!(!point_in_polygon(&[], [0.0, 0.0]));
    // A sankey band: inside its curve, not above it.
    let link = crate::layout::SankeyLinkLayout {
        source: 0,
        target: 1,
        value: 1.0,
        width: 10.0,
        x0: 0.0,
        x1: 100.0,
        y0: 10.0,
        y1: 90.0,
    };
    let band = link.band(0.25);
    assert!(point_in_polygon(&band, [50.0, 50.0]));
    assert!(point_in_polygon(&band, [2.0, 10.0]));
    assert!(!point_in_polygon(&band, [50.0, 20.0]));
    assert!(!point_in_polygon(&band, [2.0, 30.0]));
}
