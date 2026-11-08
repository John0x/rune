use super::*;

#[test]
fn tenths_come_out_exact() {
    assert_eq!(ticks(0.0, 1.0, 10), [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]);
    assert_eq!(ticks(-0.3, 0.3, 6), [-0.3, -0.2, -0.1, 0.0, 0.1, 0.2, 0.3]);
}

#[test]
fn steps_are_one_two_or_five() {
    assert_eq!(ticks(0.0, 10.0, 5), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    assert_eq!(ticks(0.0, 100.0, 3), [0.0, 50.0, 100.0]);
    assert_eq!(ticks(1.0, 9.0, 3), [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(ticks(-12.0, 47.0, 5), [-10.0, 0.0, 10.0, 20.0, 30.0, 40.0]);
    for (a, b, n) in [(0.0, 7.3, 4), (0.12, 0.98, 7), (1e6, 3.4e6, 8), (-5e-4, 2e-3, 5)] {
        let step = tick_step(a, b, n);
        let mantissa = step / 10f64.powf(step.log10().floor());
        assert!([1.0, 2.0, 5.0].iter().any(|m| (mantissa - m).abs() < 1e-9), "{step} for {a}..{b}");
    }
}

#[test]
fn reversed_domains_tick_downward() {
    assert_eq!(ticks(1.0, 0.0, 5), [1.0, 0.8, 0.6, 0.4, 0.2, 0.0]);
    assert_eq!(tick_step(1.0, 0.0, 5), -0.2);
    assert_eq!(tick_step(0.0, 1.0, 5), 0.2);
}

#[test]
fn odd_inputs_are_safe() {
    assert_eq!(ticks(3.0, 3.0, 5), [3.0]);
    assert!(ticks(0.0, 1.0, 0).is_empty());
    assert!(ticks(f64::NAN, 1.0, 5).is_empty());
    assert!(ticks(0.0, f64::INFINITY, 5).is_empty());
    assert_eq!(tick_step(3.0, 3.0, 5), 0.0);
    assert_eq!(tick_step(0.0, f64::NAN, 5), 0.0);
    let many = ticks(0.0, 1.0, usize::MAX);
    assert!(!many.is_empty() && many.len() <= MAX_TICKS);
    assert_eq!(ticks(1e300, 1e301, 9).len(), 10);
    assert_eq!(ticks(1e9, 1e9 + 1e-3, 2), [1e9, 1e9 + 5e-4, 1e9 + 1e-3]);
}

#[test]
fn nice_extends_to_whole_steps() {
    assert_eq!(nice(0.13, 0.97, 10), (0.1, 1.0));
    assert_eq!(nice(0.97, 0.13, 10), (1.0, 0.1));
    assert_eq!(nice(1.1, 10.9, 10), (1.0, 11.0));
    assert_eq!(nice(-3.7, 81.2, 5), (-20.0, 100.0));
    assert_eq!(nice(0.0, 1.0, 10), (0.0, 1.0));
    assert_eq!(nice(5.0, 5.0, 10), (5.0, 5.0));
    assert_eq!(nice(0.0, 1.0, 0), (0.0, 1.0));
    let (a, b) = nice(f64::NAN, 1.0, 10);
    assert!(a.is_nan() && b == 1.0);
}

#[test]
fn nice_domains_start_and_end_on_ticks() {
    for (a, b) in [(0.13, 0.97), (-3.7, 81.2), (17.0, 1234.0), (1e-7, 3.3e-7)] {
        let (lo, hi) = nice(a, b, 6);
        let t = ticks(lo, hi, 6);
        assert_eq!((t[0], t[t.len() - 1]), (lo, hi), "{a}..{b}");
    }
}
