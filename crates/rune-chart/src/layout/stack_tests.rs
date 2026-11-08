use super::*;

#[test]
fn series_stack_on_each_other() {
    let s = stack(&[vec![1.0, 2.0], vec![3.0, 4.0]], StackOrder::None, StackOffset::None);
    assert_eq!(s, [vec![[0.0, 1.0], [0.0, 2.0]], vec![[1.0, 4.0], [2.0, 6.0]]]);
    let negative = stack(&[[1.0], [-3.0]], StackOrder::None, StackOffset::None);
    assert_eq!(negative, [vec![[0.0, 1.0]], vec![[1.0, -2.0]]], "without diverging, negatives go back down");
}

#[test]
fn orders_pick_the_bottom_series() {
    let data = [vec![5.0, 5.0], vec![1.0, 1.0], vec![3.0, 3.0]];
    assert_eq!(StackOrder::None.apply(&data), [0, 1, 2]);
    assert_eq!(StackOrder::Reverse.apply(&data), [2, 1, 0]);
    assert_eq!(StackOrder::Ascending.apply(&data), [1, 2, 0]);
    assert_eq!(StackOrder::Descending.apply(&data), [0, 2, 1]);
    let s = stack(&data, StackOrder::Ascending, StackOffset::None);
    assert_eq!((s[1][0], s[2][0], s[0][0]), ([0.0, 1.0], [1.0, 4.0], [4.0, 9.0]));
}

#[test]
fn inside_out_puts_early_peaks_in_the_middle() {
    let data = [
        vec![0.0, 0.0, 1.0],
        vec![3.0, 2.0, 0.0],
        vec![0.0, 4.0, 0.0],
        vec![0.0, 0.0, 2.0],
        vec![0.0, 0.0, 1.0],
        vec![0.0, 0.0, 0.0],
    ];
    let order = StackOrder::InsideOut.apply(&data);
    assert_eq!(order, [3, 1, 5, 2, 0, 4]);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn expand_normalizes_each_index() {
    let s = stack(&[vec![1.0, 0.0, 2.0], vec![3.0, 0.0, 2.0]], StackOrder::None, StackOffset::Expand);
    assert_eq!(s[0][0], [0.0, 0.25]);
    assert_eq!(s[1][0], [0.25, 1.0]);
    assert_eq!(s[1][1], [0.0, 0.0], "an all-zero index stays at zero");
    assert_eq!(s[1][2], [0.5, 1.0]);
}

#[test]
fn diverging_splits_signs_around_zero() {
    let s = stack(&[vec![1.0, -1.0], vec![2.0, -2.0], vec![-1.0, 3.0]], StackOrder::None, StackOffset::Diverging);
    assert_eq!(s[0], [[0.0, 1.0], [-1.0, 0.0]]);
    assert_eq!(s[1], [[1.0, 3.0], [-3.0, -1.0]]);
    assert_eq!(s[2], [[-1.0, 0.0], [0.0, 3.0]]);
}

#[test]
fn gaps_and_ragged_series() {
    let s = stack(&[vec![1.0, f64::NAN, 1.0], vec![2.0, 2.0]], StackOrder::None, StackOffset::None);
    assert!(s[0][1][0].is_nan() && s[0][1][1].is_nan());
    assert_eq!(s[1][1], [0.0, 2.0], "the series above rests on what is below the gap");
    assert!(s[1][2][0].is_nan(), "missing values are gaps");
    assert_eq!(s[0][2], [0.0, 1.0]);
    assert!(stack::<Vec<f64>>(&[], StackOrder::InsideOut, StackOffset::Expand).is_empty());
    assert_eq!(stack(&[Vec::<f64>::new()], StackOrder::None, StackOffset::None), [Vec::<[f64; 2]>::new()]);
}

#[test]
fn bounds_line_up_with_the_series_below() {
    let data = [vec![2.0, 1.0, 4.0], vec![1.0, 1.0, 1.0], vec![3.0, 0.5, 2.0]];
    for order in
        [StackOrder::None, StackOrder::Reverse, StackOrder::Ascending, StackOrder::Descending, StackOrder::InsideOut]
    {
        let s = stack(&data, order, StackOffset::None);
        let o = order.apply(&data);
        assert!(s[o[0]].iter().all(|b| b[0] == 0.0));
        for w in o.windows(2) {
            for (below, above) in s[w[0]].iter().zip(&s[w[1]]) {
                assert_eq!(below[1], above[0], "{order:?}");
            }
        }
    }
}
