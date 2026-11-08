use rune::oklch;

use super::*;

fn close(a: Color, b: Color) -> bool {
    let (p, q) = (a.to_oklab(), b.to_oklab());
    (0..4).all(|i| (p[i] - q[i]).abs() < 2e-3)
}

#[test]
fn a_ramp_runs_through_its_stops() {
    let (lo, mid, hi) = (oklch(0.4, 0.15, 250.0), oklch(0.95, 0.0, 0.0), oklch(0.6, 0.2, 25.0));
    let ramp = Ramp(vec![lo, mid, hi]);
    assert!(close(ramp.at(0.0), lo));
    assert!(close(ramp.at(0.5), mid));
    assert!(close(ramp.at(1.0), hi));
    assert!(close(ramp.at(0.25), mix_oklab(lo, mid, 0.5)));
    assert!(close(ramp.at(0.75), mix_oklab(mid, hi, 0.5)));
    assert!(close(ramp.at(-1.0), lo));
    assert!(close(ramp.at(7.0), hi));
}

#[test]
fn short_ramps() {
    assert_eq!(Ramp(Vec::new()).at(0.5), Color::TRANSPARENT);
    assert_eq!(Ramp(vec![Color::WHITE]).at(0.9), Color::WHITE);
}

#[test]
fn fractions_clamp_to_the_domain() {
    assert_eq!(fraction(5.0, (0.0, 10.0)), 0.5);
    assert_eq!(fraction(-5.0, (0.0, 10.0)), 0.0);
    assert_eq!(fraction(50.0, (0.0, 10.0)), 1.0);
    assert_eq!(fraction(3.0, (3.0, 3.0)), 0.5);
}

#[test]
fn column_labels_thin_to_fit() {
    // 24 hours over 552 points: two-digit labels fit every column.
    assert_eq!(label_stride(23.0, 13.0, 6.0), 1);
    // `00:00` labels need every other column.
    assert_eq!(label_stride(23.0, 28.0, 6.0), 2);
    assert_eq!(label_stride(5.0, 28.0, 6.0), 7);
    assert_eq!(label_stride(0.0, 28.0, 6.0), 1);
    assert_eq!(label_stride(f32::NAN, 28.0, 6.0), 1);
}

#[test]
fn rounded_rects_stay_in_their_box() {
    let r = rounded_rect(10.0, 20.0, 30.0, 16.0, 4.0);
    assert_eq!(r.len(), 20);
    for p in &r {
        assert!(p[0] >= 10.0 - 1e-4 && p[0] <= 40.0 + 1e-4 && p[1] >= 20.0 - 1e-4 && p[1] <= 36.0 + 1e-4, "{p:?}");
    }
    // Starts at the top edge, after the top-left corner, and goes clockwise.
    assert!((r[0][0] - 36.0).abs() < 1e-4 && (r[0][1] - 20.0).abs() < 1e-4);
    assert!((r[4][0] - 40.0).abs() < 1e-4 && (r[4][1] - 24.0).abs() < 1e-4);
}

#[test]
fn sharp_and_tiny_rects() {
    assert_eq!(rounded_rect(0.0, 0.0, 4.0, 2.0, 0.0), vec![[0.0, 0.0], [4.0, 0.0], [4.0, 2.0], [0.0, 2.0]]);
    // The radius never passes half the shorter side.
    let r = rounded_rect(0.0, 0.0, 10.0, 2.0, 8.0);
    assert!(r.iter().all(|p| p[1] >= -1e-4 && p[1] <= 2.0 + 1e-4));
}
