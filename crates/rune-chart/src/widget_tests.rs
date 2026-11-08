use rune::{Color, oklch};

use super::*;

fn close(a: Color, b: Color) -> bool {
    let (p, q) = (a.to_oklab(), b.to_oklab());
    (0..4).all(|i| (p[i] - q[i]).abs() < 2e-3)
}

#[test]
fn mixing_keeps_the_ends() {
    let (a, b) = (oklch(0.3, 0.1, 250.0), oklch(0.9, 0.15, 80.0));
    assert!(close(mix_oklab(a, b, 0.0), a));
    assert!(close(mix_oklab(a, b, 1.0), b));
    assert!(close(mix_oklab(a, b, -1.0), a));
    assert!(close(mix_oklab(a, b, 2.0), b));
    assert!(close(mix_oklab(a, b, f32::NAN), a));
}

#[test]
fn mixing_runs_evenly_in_lightness() {
    let (a, b) = (oklch(0.2, 0.0, 0.0), oklch(0.8, 0.0, 0.0));
    let mid = mix_oklab(a, b, 0.5).to_oklab()[0];
    assert!((mid - 0.5).abs() < 2e-3, "{mid}");
    // Straight in linear light the middle would be far lighter.
    assert!(a.mix(b, 0.5).to_oklab()[0] > 0.6);
}

#[test]
fn mixing_mixes_alpha() {
    let m = mix_oklab(Color::WHITE.alpha(0.0), Color::WHITE, 0.25);
    assert!((m.a - 0.25).abs() < 1e-6);
}

#[test]
fn the_default_format_keeps_three_digits() {
    let f = default_format();
    assert_eq!(f(0.0), "0");
    assert_eq!(f(42.0), "42");
    assert_eq!(f(4.56789), "4.57");
    assert_eq!(f(1234.0), "1.23k");
    assert_eq!(f(2_500_000.0), "2.5M");
    assert_eq!(f(-0.5), "-0.5");
}
