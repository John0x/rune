use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn slices_are_proportional_and_contiguous() {
    let s = pie(&[1.0, 2.0, 3.0, 4.0], &PieOptions::default());
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].start, 0.0);
    assert!(close(s[3].end, TAU));
    for w in s.windows(2) {
        assert_eq!(w[0].end, w[1].start);
    }
    for (i, slice) in s.iter().enumerate() {
        assert_eq!(slice.index, i);
        assert!(close(slice.end - slice.start, TAU * (i + 1) as f64 / 10.0));
    }
}

#[test]
fn sorting_changes_angles_not_order() {
    let s = pie(&[1.0, 3.0, 2.0], &PieOptions { sort: PieSort::Descending, ..PieOptions::default() });
    assert_eq!(s.iter().map(|s| s.index).collect::<Vec<_>>(), [0, 1, 2], "results stay in input order");
    assert_eq!(s[1].start, 0.0, "the largest goes first");
    assert_eq!(s[2].start, s[1].end);
    assert!(close(s[0].end, TAU));
    let up = pie(&[1.0, 3.0, 2.0], &PieOptions { sort: PieSort::Ascending, ..PieOptions::default() });
    assert_eq!(up[0].start, 0.0);
    assert_eq!(up[2].start, up[0].end);
}

#[test]
fn padding_is_shared_out() {
    let pad = 0.1;
    let s = pie(&[1.0, 1.0, 2.0], &PieOptions { pad_angle: pad, ..PieOptions::default() });
    assert!(close(s[2].end, TAU));
    assert!(s.iter().all(|s| s.pad == pad));
    let k = (TAU - 3.0 * pad) / 4.0;
    assert!(close(s[0].end - s[0].start, k + pad));
    assert!(close(s[2].end - s[2].start, 2.0 * k + pad));
    let greedy = pie(&[1.0, 1.0], &PieOptions { pad_angle: 10.0, ..PieOptions::default() });
    assert!(close(greedy[0].pad, PI), "at most an equal share");
}

#[test]
fn partial_and_counterclockwise_pies() {
    let half = pie(&[1.0, 1.0], &PieOptions { start_angle: -FRAC_PI_2, end_angle: FRAC_PI_2, ..PieOptions::default() });
    assert_eq!((half[0].start, half[0].end, half[1].end), (-FRAC_PI_2, 0.0, FRAC_PI_2));
    let ccw = pie(&[1.0, 1.0], &PieOptions { end_angle: -TAU, ..PieOptions::default() });
    assert_eq!((ccw[0].end, ccw[1].end), (-PI, -TAU));
    let over = pie(&[1.0], &PieOptions { end_angle: 100.0, ..PieOptions::default() });
    assert!(close(over[0].end, TAU), "at most a full turn");
}

#[test]
fn odd_values_count_as_zero() {
    let s = pie(&[f64::NAN, -2.0, 3.0, f64::INFINITY], &PieOptions::default());
    assert_eq!(s.iter().map(|s| s.value).collect::<Vec<_>>(), [0.0, 0.0, 3.0, 0.0]);
    assert!(close(s[2].end - s[2].start, TAU));
    let zeros = pie(&[0.0, 0.0], &PieOptions { pad_angle: 0.2, ..PieOptions::default() });
    assert!(zeros.iter().all(|s| close(s.end - s.start, 0.2)));
    assert!(pie(&[], &PieOptions::default()).is_empty());
    let nan_opts = PieOptions { start_angle: f64::NAN, end_angle: f64::NAN, pad_angle: f64::NAN, sort: PieSort::None };
    let s = pie(&[1.0, 1.0], &nan_opts);
    assert_eq!((s[0].start, s[1].end, s[0].pad), (0.0, TAU, 0.0));
}
