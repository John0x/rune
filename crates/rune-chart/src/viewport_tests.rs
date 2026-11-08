use super::*;

const SIZE: [f32; 2] = [200.0, 100.0];

fn view() -> Viewport {
    Viewport::new((0.0, 100.0), (0.0, 10.0))
}

#[test]
fn pixels_map_to_data_with_y_up() {
    let v = view();
    assert_eq!(v.point_at([0.0, 0.0], SIZE), (0.0, 10.0));
    assert_eq!(v.point_at([200.0, 100.0], SIZE), (100.0, 0.0));
    assert_eq!(v.point_at([50.0, 25.0], SIZE), (25.0, 7.5));
    assert_eq!(v.point_at([50.0, 25.0], [0.0, 0.0]), (50.0, 5.0));
}

#[test]
fn zoom_keeps_the_anchor_under_the_pointer() {
    let v = view();
    let px = [50.0, 25.0];
    let anchor = v.point_at(px, SIZE);
    let z = v.zoom(anchor, 2.0, Axes::Both);
    assert_eq!((z.span_x(), z.span_y()), (50.0, 5.0));
    assert_eq!(z.point_at(px, SIZE), anchor);
    let out = z.zoom(anchor, 0.5, Axes::Both);
    assert_eq!(out, v);
}

#[test]
fn zoom_on_one_axis_and_bad_factors() {
    let v = view();
    let x = v.zoom((50.0, 5.0), 4.0, Axes::X);
    assert_eq!(x, Viewport::new((37.5, 62.5), (0.0, 10.0)));
    let y = v.zoom((50.0, 0.0), 2.0, Axes::Y);
    assert_eq!(y, Viewport::new((0.0, 100.0), (0.0, 5.0)));
    for f in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(v.zoom((50.0, 5.0), f, Axes::Both), v);
    }
    assert_eq!(v.zoom((f64::NAN, 5.0), 2.0, Axes::Both).x, v.x);
}

#[test]
fn pan_follows_the_drag() {
    let v = view();
    let right = v.pan([20.0, 0.0], SIZE, Axes::Both);
    assert_eq!(right.x, (-10.0, 90.0), "dragging right shows earlier x");
    let down = v.pan([0.0, 10.0], SIZE, Axes::Both);
    assert_eq!(down.y, (1.0, 11.0), "dragging down shows higher y");
    assert_eq!(v.pan([20.0, 10.0], SIZE, Axes::X).y, v.y);
    assert_eq!(v.pan([20.0, 10.0], [0.0, 0.0], Axes::Both), v);
    assert_eq!(v.pan([f32::NAN, 0.0], SIZE, Axes::Both), v);
}

#[test]
fn box_zoom_takes_corners_in_any_order() {
    let v = view();
    let b = v.box_zoom([150.0, 75.0], [50.0, 25.0], SIZE, Axes::Both);
    assert_eq!(b, Viewport::new((25.0, 75.0), (2.5, 7.5)));
    let flat = v.box_zoom([50.0, 40.0], [150.0, 40.5], SIZE, Axes::Both);
    assert_eq!(flat, Viewport::new((25.0, 75.0), (0.0, 10.0)), "a sliver keeps y");
    assert_eq!(v.box_zoom([50.0, 25.0], [150.0, 75.0], SIZE, Axes::X).y, v.y);
}

#[test]
fn fit_pads_and_widens_flat_extents() {
    assert_eq!(Viewport::fit((0.0, 10.0), (0.0, 100.0), (0.0, 0.1)), Viewport::new((0.0, 10.0), (-10.0, 110.0)));
    assert_eq!(Viewport::fit((10.0, 0.0), (5.0, 5.0), (0.0, 0.1)), Viewport::new((0.0, 10.0), (4.5, 5.5)));
    assert_eq!(Viewport::fit((10.0, 10.0), (0.0, 0.0), (0.0, 0.0)), Viewport::new((9.0, 11.0), (-1.0, 1.0)));
}

#[test]
fn fit_series_covers_all_points() {
    let a = Series::new(vec![0.0, 5.0], vec![1.0, 2.0]).unwrap();
    let b = Series::new(vec![2.0, 9.0], vec![-3.0, f64::NAN]).unwrap();
    let gaps = Series::new(vec![-100.0], vec![f64::NAN]).unwrap();
    assert_eq!(Viewport::fit_series([&a, &b, &gaps], (0.0, 0.0)), Some(Viewport::new((0.0, 9.0), (-3.0, 2.0))));
    assert_eq!(Viewport::fit_series([&gaps], (0.0, 0.0)), None);
    assert_eq!(Viewport::fit_series([], (0.0, 0.0)), None);
}

#[test]
fn clamp_shifts_inside_the_bounds() {
    let limits = Limits { x: Some((0.0, 100.0)), y: None, min_span: (0.0, 0.0) };
    let v = Viewport::new((-20.0, 30.0), (0.0, 1.0)).clamp(&limits);
    assert_eq!(v.x, (0.0, 50.0));
    let v = Viewport::new((80.0, 130.0), (-5.0, 1e9)).clamp(&limits);
    assert_eq!(v, Viewport::new((50.0, 100.0), (-5.0, 1e9)), "y is unbounded");
    let wide = Viewport::new((-50.0, 250.0), (0.0, 1.0)).clamp(&limits);
    assert_eq!(wide.x, (0.0, 100.0));
    assert_eq!(Viewport::new((100.0, 0.0), (0.0, 1.0)).clamp(&limits).x, (0.0, 100.0));
}

#[test]
fn clamp_enforces_the_minimum_span() {
    let limits = Limits::default().min_span(10.0, 1.0);
    let v = Viewport::new((50.0, 51.0), (3.0, 3.0)).clamp(&limits);
    assert_eq!(v, Viewport::new((45.5, 55.5), (2.5, 3.5)));
    let bounded = Limits { x: Some((0.0, 100.0)), ..limits };
    assert_eq!(Viewport::new((0.0, 1.0), (0.0, 1.0)).clamp(&bounded).x, (0.0, 10.0));
    let tight = Limits { x: Some((0.0, 4.0)), ..limits };
    assert_eq!(Viewport::new((1.0, 2.0), (0.0, 1.0)).clamp(&tight).x, (-3.0, 7.0), "the minimum span wins");
}

#[test]
fn clamp_repairs_broken_viewports() {
    let limits = Limits { x: Some((0.0, 10.0)), y: None, min_span: (f64::NAN, -1.0) };
    let v = Viewport::new((f64::NAN, 3.0), (f64::NEG_INFINITY, 2.0)).clamp(&limits);
    assert_eq!(v.x, (0.0, 10.0));
    assert!(v.y.0.is_infinite());
}

#[test]
fn limits_around_the_data() {
    let l = Limits::around(Viewport::new((0.0, 100.0), (10.0, 0.0)), 0.1).min_span(1.0, 0.5);
    assert_eq!(l, Limits { x: Some((-10.0, 110.0)), y: Some((-1.0, 11.0)), min_span: (1.0, 0.5) });
}

#[test]
fn y_follows_the_visible_points() {
    let s = Series::new(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0], vec![100.0, 2.0, f64::NAN, 4.0, 3.0, -50.0]).unwrap();
    let flat = Series::new(vec![2.5], vec![1.0]).unwrap();
    let v = Viewport::new((1.0, 4.0), (0.0, 1.0)).fit_y_to_visible([&s], 0.0);
    assert_eq!(v.y, (2.0, 4.0), "spikes outside the view don't count");
    let v = Viewport::new((1.0, 4.0), (0.0, 1.0)).fit_y_to_visible([&s, &flat], 0.5);
    assert_eq!(v.y, (-0.5, 5.5));
    let empty = Viewport::new((10.0, 20.0), (0.0, 1.0));
    assert_eq!(empty.fit_y_to_visible([&s], 0.1), empty);
    let reversed = Viewport::new((4.0, 1.0), (0.0, 1.0)).fit_y_to_visible([&s], 0.0);
    assert_eq!(reversed.y, (2.0, 4.0));
}

#[test]
fn lerp_and_approx_eq_drive_animations() {
    let a = view();
    let b = Viewport::new((50.0, 60.0), (5.0, 6.0));
    assert_eq!(a.lerp(b, 0.0), a);
    assert_eq!(a.lerp(b, 1.0), b);
    assert_eq!(a.lerp(b, 0.5), Viewport::new((25.0, 80.0), (2.5, 8.0)));
    assert!(a.approx_eq(&Viewport::new((0.05, 100.0), (0.0, 10.005)), 1e-3));
    assert!(!a.approx_eq(&Viewport::new((0.5, 100.0), (0.0, 10.0)), 1e-3));
    assert!(!a.approx_eq(&Viewport::new((f64::NAN, 100.0), (0.0, 10.0)), 1e-3));
    let flat = Viewport::new((1.0, 1.0), (1.0, 1.0));
    assert!(flat.approx_eq(&flat, 1e-3));
}
