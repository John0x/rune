//! Pure geometry shared by areas, bars and candles: bar widths, bar outlines, candle bodies,
//! merging what is too thin to tell apart per pixel column, and splitting a line where it
//! crosses a baseline. Pixel values are the plot area's points, y growing downward.

use std::ops::Range;

use crate::layout::{StackOffset, StackOrder, stack};

/// The smallest gap between neighbouring distinct values over the ascending columns `xs`, or
/// `None` when no column has two distinct values.
pub(crate) fn min_spacing<'a>(xs: impl IntoIterator<Item = &'a [f64]>) -> Option<f64> {
    xs.into_iter()
        .flat_map(|x| x.windows(2).map(|w| w[1] - w[0]))
        .filter(|d| d.is_finite() && *d > 0.0)
        .reduce(f64::min)
}

/// A bar's width in points: `fraction` of the smallest x spacing in points, or of `fallback`
/// when there is no spacing (a single bar), and at least 1.
pub(crate) fn bar_width(spacing: Option<f32>, fraction: f32, fallback: f32) -> f32 {
    let fraction = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.7 };
    let room = spacing.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(fallback);
    (room * fraction).max(1.0)
}

/// The outline of a bar from x `left` to `right` and from y `from` (its base) to `to` (its
/// value), with the two corners at `to` rounded by `radius`, which shrinks to fit half the width
/// and the height.
pub(crate) fn bar_outline(left: f32, right: f32, from: f32, to: f32, radius: f32) -> Vec<[f32; 2]> {
    let r = radius.min((right - left) / 2.0).min((to - from).abs()).max(0.0);
    if r < 0.5 {
        return vec![[left, from], [right, from], [right, to], [left, to]];
    }
    // `dir` points from the base toward the value: -1 for a bar going up the screen.
    let dir = if to < from { -1.0 } else { 1.0 };
    let mut out = vec![[left, from], [right, from]];
    let arc = |out: &mut Vec<[f32; 2]>, cx: f32, start: f32| {
        for k in 0..=4 {
            let a = start + k as f32 * std::f32::consts::FRAC_PI_8;
            out.push([cx + r * a.cos(), to - dir * r + dir * r * a.sin()]);
        }
    };
    arc(&mut out, right - r, 0.0);
    arc(&mut out, left + r, std::f32::consts::FRAC_PI_2);
    out
}

/// A candle at pixel x `center`, `width` wide, from its open, high, low and close in points:
/// its wick (high to low) and its body (`[left, top, right, bottom]`, at least a point tall).
pub(crate) fn candle(center: f32, width: f32, [o, h, l, c]: [f32; 4]) -> ([[f32; 2]; 2], [f32; 4]) {
    let (mut top, mut bottom) = (o.min(c), o.max(c));
    if bottom - top < 1.0 {
        let mid = (top + bottom) / 2.0;
        (top, bottom) = (mid - 0.5, mid + 0.5);
    }
    let half = width / 2.0;
    ([[center, h.min(l)], [center, h.max(l)]], [center - half, top, center + half, bottom])
}

/// The indices of `range` grouped by the whole pixel column their x falls in (`to_px` maps an x
/// to points), in order, so bars or candles too thin to tell apart merge into one per column.
pub(crate) fn column_groups(x: &[f64], range: Range<usize>, to_px: impl Fn(f64) -> f32) -> Vec<Range<usize>> {
    let end = range.end.min(x.len());
    let mut groups: Vec<Range<usize>> = Vec::new();
    let mut column = f32::NAN;
    for (i, &v) in x.iter().enumerate().take(end).skip(range.start) {
        let c = to_px(v).floor();
        match groups.last_mut() {
            Some(g) if c == column => g.end = i + 1,
            _ => groups.push(i..i + 1),
        }
        column = c;
    }
    groups
}

/// The open, high, low and close of the rows in `range` merged: the first finite open, the
/// highest high, the lowest low and the last finite close; `None` without a complete row.
pub(crate) fn merge_ohlc(
    open: &[f64],
    high: &[f64],
    low: &[f64],
    close: &[f64],
    range: Range<usize>,
) -> Option<[f64; 4]> {
    let complete = |i: &usize| [open[*i], high[*i], low[*i], close[*i]].iter().all(|v| v.is_finite());
    let first = range.clone().find(complete)?;
    let last = range.clone().rev().find(complete)?;
    let rows = range.filter(complete);
    let (h, l) = rows.fold((f64::NEG_INFINITY, f64::INFINITY), |(h, l), i| (h.max(high[i]), l.min(low[i])));
    Some([open[first], h, l, close[last]])
}

/// A polyline split into runs that each lie on one side of the horizontal line at y `base`,
/// with the crossing points added to both runs it joins, so each run fills to the baseline as
/// one band. Each run comes with whether it lies above the line (y at most `base`).
pub(crate) fn split_at(points: &[[f32; 2]], base: f32) -> Vec<(bool, Vec<[f32; 2]>)> {
    let mut out: Vec<(bool, Vec<[f32; 2]>)> = Vec::new();
    for (i, &p) in points.iter().enumerate() {
        let above = p[1] <= base;
        match out.last_mut() {
            Some((side, run)) if *side == above => run.push(p),
            Some((_, run)) => {
                let q = points[i - 1];
                let t = (base - q[1]) / (p[1] - q[1]);
                let cross = [q[0] + (p[0] - q[0]) * t, base];
                run.push(cross);
                out.push((above, vec![cross, p]));
            }
            None => out.push((above, vec![p])),
        }
    }
    out
}

/// `y` in points kept inside a plot area `height` tall; a value off the scale (a log scale's
/// zero) goes to the bottom.
pub(crate) fn clamp_y(y: f32, height: f32) -> f32 {
    if y.is_finite() { y.clamp(0.0, height) } else { height }
}

/// The layers' values at `indices` stacked as [`stack`] does, bottom layer first: per layer,
/// `[lower, upper]` per index. A hidden layer, and a value past a layer's end, is a gap
/// (`[NaN, NaN]`) that the layers above close over.
pub(crate) fn stack_at<V: AsRef<[f64]>>(
    values: &[V],
    hidden: &[bool],
    indices: &[usize],
    offset: StackOffset,
) -> Vec<Vec<[f64; 2]>> {
    let columns: Vec<Vec<f64>> = values
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let shown = !hidden.get(k).copied().unwrap_or(false);
            indices.iter().map(|&i| v.as_ref().get(i).copied().filter(|_| shown).unwrap_or(f64::NAN)).collect()
        })
        .collect();
    stack(&columns, StackOrder::None, offset)
}

/// Index lists merged into one ascending list without repeats.
pub(crate) fn merge_indices(lists: impl IntoIterator<Item = Vec<usize>>) -> Vec<usize> {
    let mut all: Vec<usize> = lists.into_iter().flatten().collect();
    all.sort_unstable();
    all.dedup();
    all
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
