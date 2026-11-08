use super::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

#[test]
fn decades_take_equal_space() {
    let s = Log::new((1.0, 100.0), (0.0, 200.0));
    assert_eq!(s.map(1.0), 0.0);
    assert_eq!(s.map(10.0), 100.0);
    assert_eq!(s.map(100.0), 200.0);
    assert!(close(s.invert(100.0), 10.0));
    assert!(close(s.invert(50.0), 10f64.sqrt()));
}

#[test]
fn non_positive_values_map_to_nan() {
    let s = Log::new((1.0, 100.0), (0.0, 200.0));
    assert!(s.map(0.0).is_nan());
    assert!(s.map(-3.0).is_nan());
    assert!(s.map(f64::NAN).is_nan());
}

#[test]
fn invalid_domains_are_replaced() {
    assert_eq!(Log::new((0.0, 100.0), (0.0, 1.0)).domain(), (10.0, 100.0));
    assert_eq!(Log::new((5.0, -1.0), (0.0, 1.0)).domain(), (5.0, 50.0));
    assert_eq!(Log::new((f64::NAN, 0.0), (0.0, 1.0)).domain(), (1.0, 10.0));
    assert_eq!(Log::new((1.0, 10.0), (0.0, 1.0)).with_domain((-1.0, -2.0)).domain(), (1.0, 10.0));
}

#[test]
fn reversed_ranges_and_clamping() {
    let s = Log::new((1.0, 1000.0), (300.0, 0.0)).clamp(true);
    assert_eq!(s.map(10.0), 200.0);
    assert_eq!(s.map(1e6), 0.0);
    assert_eq!(s.map(0.01), 300.0);
    assert!(close(s.invert(-50.0), 1000.0));
    assert_eq!(s.with_range((0.0, 3.0)).range(), (0.0, 3.0));
}

#[test]
fn few_decades_tick_every_multiple() {
    let t = Log::new((1.0, 100.0), (0.0, 1.0)).ticks(10);
    let expected: Vec<f64> = (1..10).map(f64::from).chain((1..=10).map(|k| f64::from(k) * 10.0)).collect();
    assert_eq!(t, expected);
    let small = Log::new((0.02, 0.5), (0.0, 1.0)).ticks(10);
    assert_eq!(small, [0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08, 0.09, 0.1, 0.2, 0.3, 0.4, 0.5]);
}

#[test]
fn many_decades_tick_some_powers() {
    let t = Log::new((1.0, 1e20), (0.0, 1.0)).ticks(5);
    assert!(t.len() <= 6 && t.len() >= 3);
    assert!(t.iter().all(|v| close(v.log10().round(), v.log10())));
    assert_eq!(t[0], 1.0);
}

#[test]
fn narrow_domains_fall_back_to_linear_ticks() {
    let t = Log::new((1.0, 3.0), (0.0, 1.0)).ticks(10);
    assert_eq!(t.first(), Some(&1.0));
    assert_eq!(t.last(), Some(&3.0));
    assert!(t.len() >= 10);
}

#[test]
fn ticks_follow_the_domain_direction_and_odd_counts() {
    let t = Log::new((1000.0, 1.0), (0.0, 1.0)).ticks(3);
    assert!(t.windows(2).all(|w| w[0] > w[1]));
    assert!(Log::new((1.0, 10.0), (0.0, 1.0)).ticks(0).is_empty());
    assert_eq!(Log::new((5.0, 5.0), (0.0, 1.0)).ticks(5), [5.0]);
}

#[test]
fn nice_rounds_to_powers_of_ten() {
    assert_eq!(Log::new((2.0, 300.0), (0.0, 1.0)).nice().domain(), (1.0, 1000.0));
    assert_eq!(Log::new((300.0, 2.0), (0.0, 1.0)).nice().domain(), (1000.0, 1.0));
    assert_eq!(Log::new((0.03, 0.5), (0.0, 1.0)).nice().domain(), (0.01, 1.0));
}

#[test]
fn labels_keep_their_own_precision() {
    let s = Log::new((0.01, 1e7), (0.0, 1.0));
    let label = |v| Continuous::format_tick(&s, v, 10);
    assert_eq!(label(0.02), "0.02");
    assert_eq!(label(5.0), "5");
    assert_eq!(label(3000.0), "3k");
    assert_eq!(label(2e7), "20M");
}
