use super::*;

#[test]
fn spacing_is_the_smallest_gap_over_every_column() {
    assert_eq!(min_spacing([[0.0, 2.0, 3.0].as_slice(), &[10.0, 10.5]]), Some(0.5));
    assert_eq!(min_spacing([[1.0, 1.0, 4.0].as_slice()]), Some(3.0), "equal neighbours are skipped");
    assert_eq!(min_spacing([[1.0].as_slice(), &[]]), None);
}

#[test]
fn bar_width_is_a_fraction_of_the_spacing_and_at_least_a_point() {
    assert_eq!(bar_width(Some(20.0), 0.7, 100.0), 14.0);
    assert_eq!(bar_width(Some(0.5), 0.7, 100.0), 1.0);
    assert_eq!(bar_width(None, 0.5, 60.0), 30.0, "a single bar takes the fallback");
    assert_eq!(bar_width(Some(f32::NAN), 0.5, 60.0), 30.0);
    assert_eq!(bar_width(Some(10.0), 3.0, 60.0), 10.0, "the fraction is at most 1");
}

#[test]
fn bar_outline_rounds_the_value_end() {
    let square = bar_outline(0.0, 10.0, 50.0, 20.0, 0.0);
    assert_eq!(square, [[0.0, 50.0], [10.0, 50.0], [10.0, 20.0], [0.0, 20.0]]);
    let up = bar_outline(0.0, 10.0, 50.0, 20.0, 3.0);
    assert_eq!(up.len(), 12);
    let top = up.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    assert!((top - 20.0).abs() < 1e-4, "the rounded end reaches the value");
    assert!(up.iter().all(|p| p[0] >= -1e-4 && p[0] <= 10.0 + 1e-4 && p[1] <= 50.0 + 1e-4));
    let down = bar_outline(0.0, 10.0, 50.0, 80.0, 3.0);
    let bottom = down.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    assert!((bottom - 80.0).abs() < 1e-4, "a negative bar rounds its bottom");
    let short = bar_outline(0.0, 10.0, 50.0, 49.8, 3.0);
    assert_eq!(short.len(), 4, "a radius taller than the bar goes square");
}

#[test]
fn candle_body_spans_open_and_close() {
    let (wick, body) = candle(100.0, 8.0, [40.0, 10.0, 60.0, 20.0]);
    assert_eq!(wick, [[100.0, 10.0], [100.0, 60.0]]);
    assert_eq!(body, [96.0, 20.0, 104.0, 40.0]);
    let (_, doji) = candle(0.0, 4.0, [30.0, 20.0, 40.0, 30.0]);
    assert_eq!(doji, [-2.0, 29.5, 2.0, 30.5], "an unchanged price still shows a point-tall body");
}

#[test]
fn column_groups_merge_rows_in_one_pixel() {
    let x = [0.0, 0.2, 0.9, 1.1, 3.0, 3.5];
    let groups = column_groups(&x, 0..6, |v| v as f32);
    assert_eq!(groups, [0..3, 3..4, 4..6]);
    assert_eq!(column_groups(&x, 2..4, |v| v as f32), [2..3, 3..4]);
    assert_eq!(column_groups(&x, 4..99, |v| v as f32), vec![4..6], "a range past the end is cut");
    assert!(column_groups(&[], 0..0, |v| v as f32).is_empty());
}

#[test]
fn merged_ohlc_takes_first_open_extremes_and_last_close() {
    let (o, h, l, c) = ([1.0, 2.0, f64::NAN], [3.0, 5.0, 9.0], [0.5, 1.0, 0.1], [2.0, 4.0, 3.0]);
    assert_eq!(merge_ohlc(&o, &h, &l, &c, 0..3), Some([1.0, 5.0, 0.5, 4.0]), "the incomplete row is skipped");
    assert_eq!(merge_ohlc(&o, &h, &l, &c, 2..3), None);
}

#[test]
fn split_at_cuts_where_the_line_crosses() {
    let runs = split_at(&[[0.0, 10.0], [10.0, 30.0], [20.0, 10.0]], 20.0);
    assert_eq!(
        runs,
        [
            (true, vec![[0.0, 10.0], [5.0, 20.0]]),
            (false, vec![[5.0, 20.0], [10.0, 30.0], [15.0, 20.0]]),
            (true, vec![[15.0, 20.0], [20.0, 10.0]]),
        ]
    );
    assert_eq!(split_at(&[[0.0, 1.0], [1.0, 2.0]], 5.0), [(true, vec![[0.0, 1.0], [1.0, 2.0]])]);
    assert!(split_at(&[], 5.0).is_empty());
}

#[test]
fn clamp_y_keeps_the_baseline_in_the_plot() {
    assert_eq!(clamp_y(-40.0, 100.0), 0.0);
    assert_eq!(clamp_y(140.0, 100.0), 100.0);
    assert_eq!(clamp_y(f32::NEG_INFINITY, 100.0), 100.0);
    assert_eq!(clamp_y(f32::NAN, 100.0), 100.0);
}

#[test]
fn stack_at_stacks_the_picked_indices_and_skips_hidden_layers() {
    let values = [vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0], vec![100.0, 200.0]];
    let s = stack_at(&values, &[false, false, false], &[0, 2], StackOffset::None);
    assert_eq!(s[0], [[0.0, 1.0], [0.0, 3.0]]);
    assert_eq!(s[1], [[1.0, 11.0], [3.0, 33.0]]);
    assert_eq!(s[2][0], [11.0, 111.0]);
    assert!(s[2][1][0].is_nan(), "past a layer's end is a gap");
    let hidden = stack_at(&values, &[true, false, false], &[1], StackOffset::None);
    assert!(hidden[0][0][0].is_nan());
    assert_eq!(hidden[1][0], [0.0, 20.0], "the layer above closes over a hidden one");
    let diverging = stack_at(&[vec![2.0], vec![-3.0], vec![1.0]], &[], &[0], StackOffset::Diverging);
    assert_eq!((diverging[0][0], diverging[1][0], diverging[2][0]), ([0.0, 2.0], [-3.0, 0.0], [2.0, 3.0]));
    assert!(stack_at(&values, &[], &[], StackOffset::None).iter().all(Vec::is_empty));
}

#[test]
fn merge_indices_sorts_and_dedups() {
    assert_eq!(merge_indices([vec![3, 1], vec![2, 3], vec![]]), [1, 2, 3]);
}
