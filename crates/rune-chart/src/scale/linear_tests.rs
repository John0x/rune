use super::*;

#[test]
fn maps_and_inverts() {
    let s = Linear::new((0.0, 10.0), (0.0, 100.0));
    assert_eq!(s.map(2.5), 25.0);
    assert_eq!(s.map(-1.0), -10.0, "unclamped by default");
    assert_eq!(s.invert(75.0), 7.5);
    assert!(s.map(f64::NAN).is_nan());
}

#[test]
fn reversed_ranges_grow_upward() {
    let y = Linear::new((0.0, 100.0), (200.0, 0.0));
    assert_eq!(y.map(0.0), 200.0);
    assert_eq!(y.map(100.0), 0.0);
    assert_eq!(y.map(25.0), 150.0);
    assert_eq!(y.invert(50.0), 75.0);
    let reversed_domain = Linear::new((10.0, 0.0), (0.0, 100.0));
    assert_eq!(reversed_domain.map(10.0), 0.0);
    assert_eq!(reversed_domain.ticks(5), [10.0, 8.0, 6.0, 4.0, 2.0, 0.0]);
}

#[test]
fn degenerate_domains_and_ranges_use_the_middle() {
    let flat = Linear::new((5.0, 5.0), (0.0, 100.0));
    assert_eq!(flat.map(5.0), 50.0);
    assert_eq!(flat.map(123.0), 50.0);
    assert_eq!(flat.invert(30.0), 5.0);
    assert_eq!(flat.ticks(5), [5.0]);
    let point = Linear::new((0.0, 10.0), (40.0, 40.0));
    assert_eq!(point.invert(40.0), 5.0);
    assert_eq!(point.map(3.0), 40.0);
}

#[test]
fn clamping_keeps_values_in_range() {
    let s = Linear::new((0.0, 10.0), (100.0, 0.0)).clamp(true);
    assert!(s.is_clamped());
    assert_eq!(s.map(-5.0), 100.0);
    assert_eq!(s.map(50.0), 0.0);
    assert_eq!(s.invert(-20.0), 10.0);
    assert_eq!(s.invert(500.0), 0.0);
    assert!(s.map(f64::NAN).is_nan());
}

#[test]
fn round_trips_large_values() {
    let s = Linear::new((1.7e9, 1.7e9 + 86_400.0), (0.0, 800.0));
    for px in [0.0, 13.5, 400.0, 799.0] {
        assert!((s.map(s.invert(px)) - px).abs() < 1e-3);
    }
}

#[test]
fn nice_ticks_and_labels() {
    let s = Linear::new((0.13, 0.97), (0.0, 100.0)).nice(10);
    assert_eq!(s.domain(), (0.1, 1.0));
    assert_eq!(s.range(), (0.0, 100.0));
    assert_eq!(s.ticks(5), [0.2, 0.4, 0.6, 0.8, 1.0]);
    assert_eq!(s.tick_step(5), 0.2);
    let labels: Vec<String> = s.ticks(5).iter().map(|&v| Continuous::format_tick(&s, v, 5)).collect();
    assert_eq!(labels, ["0.2", "0.4", "0.6", "0.8", "1.0"]);
}

#[test]
fn builders_replace_one_part() {
    let s = Linear::new((0.0, 1.0), (0.0, 10.0)).with_domain((0.0, 2.0)).with_range((0.0, 20.0));
    assert_eq!((s.domain(), s.range()), ((0.0, 2.0), (0.0, 20.0)));
}

#[test]
fn works_through_the_trait() {
    fn mid<S: Continuous>(s: &S) -> f64 {
        let (a, b) = s.range();
        s.invert((a + b) / 2.0)
    }
    assert_eq!(mid(&Linear::new((0.0, 10.0), (0.0, 100.0))), 5.0);
    let boxed: Box<dyn Continuous> = Box::new(Linear::new((0.0, 10.0), (0.0, 100.0)));
    assert_eq!(boxed.map(1.0), 10.0);
    assert_eq!(boxed.domain(), (0.0, 10.0));
    assert_eq!(boxed.ticks(2), [0.0, 5.0, 10.0]);
}
