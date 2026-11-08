//! Stacked series, as d3's `stack`: each series' lower and upper bound per index.

/// The order series are stacked in, bottom first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackOrder {
    /// The input order.
    #[default]
    None,
    /// The input order reversed.
    Reverse,
    /// The series with the smallest sum at the bottom.
    Ascending,
    /// The series with the largest sum at the bottom.
    Descending,
    /// The series peaking earliest in the middle, later ones alternately above and below, so a
    /// streamgraph's layers grow outward (d3's `stackOrderInsideOut`).
    InsideOut,
}

/// How each index's stack is placed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackOffset {
    /// From zero, each series on top of the previous (negative values go back down).
    #[default]
    None,
    /// Like `None`, scaled so each index's absolute values add up to 1 (100% stacks).
    Expand,
    /// Positive values stack up from zero, negative values down from zero, so bars with
    /// negatives don't overlap.
    Diverging,
}

impl StackOrder {
    /// The series indices bottom to top. Sums and peaks skip values that aren't finite.
    pub fn apply<S: AsRef<[f64]>>(self, series: &[S]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..series.len()).collect();
        let sums: Vec<f64> = series.iter().map(|s| s.as_ref().iter().filter(|v| v.is_finite()).sum()).collect();
        match self {
            StackOrder::None => {}
            StackOrder::Reverse => order.reverse(),
            StackOrder::Ascending => order.sort_by(|&a, &b| sums[a].total_cmp(&sums[b])),
            StackOrder::Descending => order.sort_by(|&a, &b| sums[b].total_cmp(&sums[a])),
            StackOrder::InsideOut => {
                let peaks: Vec<usize> = series.iter().map(|s| peak(s.as_ref())).collect();
                order.sort_by_key(|&i| peaks[i]);
                let (mut top, mut bottom) = (0.0, 0.0);
                let (mut tops, mut bottoms) = (Vec::new(), Vec::new());
                for i in order {
                    if top < bottom {
                        top += sums[i];
                        tops.push(i);
                    } else {
                        bottom += sums[i];
                        bottoms.push(i);
                    }
                }
                bottoms.reverse();
                bottoms.extend(tops);
                order = bottoms;
            }
        }
        order
    }
}

/// The index of a series' largest finite value (the first, on ties).
fn peak(values: &[f64]) -> usize {
    let mut best = (0, f64::NEG_INFINITY);
    for (i, &v) in values.iter().enumerate() {
        if v > best.1 {
            best = (i, v);
        }
    }
    best.0
}

/// Stacks `series` (one value per index each, as long as the longest; missing values count as
/// gaps) and returns, per series in input order, `[lower, upper]` per index.
///
/// A value that isn't finite is a gap: its bounds are `[NaN, NaN]`, and it adds nothing to the
/// stack, so the series above rest on the ones below it.
// Indexes by position across all series at once, which an iterator can't express.
#[allow(clippy::needless_range_loop)]
pub fn stack<S: AsRef<[f64]>>(series: &[S], order: StackOrder, offset: StackOffset) -> Vec<Vec<[f64; 2]>> {
    let len = series.iter().map(|s| s.as_ref().len()).max().unwrap_or(0);
    let order = order.apply(series);
    let mut out = vec![vec![[f64::NAN; 2]; len]; series.len()];
    let value = |i: usize, j: usize| series[i].as_ref().get(j).copied().filter(|v| v.is_finite());
    for j in 0..len {
        let scale = match offset {
            StackOffset::Expand => {
                let total: f64 = (0..series.len()).filter_map(|i| value(i, j)).map(f64::abs).sum();
                if total > 0.0 { 1.0 / total } else { 0.0 }
            }
            _ => 1.0,
        };
        let (mut up, mut down) = (0.0, 0.0);
        for &i in &order {
            let Some(v) = value(i, j) else { continue };
            let v = v * scale;
            out[i][j] = match offset {
                StackOffset::Diverging if v < 0.0 => {
                    let lower = down + v;
                    let band = [lower, down];
                    down = lower;
                    band
                }
                _ => {
                    let band = [up, up + v];
                    up += v;
                    band
                }
            };
        }
    }
    out
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
