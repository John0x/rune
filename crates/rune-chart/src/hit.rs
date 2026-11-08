//! What lies under the pointer: pure lookups from a pixel or data position to an index, for
//! hover, tooltips and selection.

use std::f64::consts::TAU;

use crate::layout::{PieSlice, angle_at};
use crate::scale::Band;

/// The index of the point whose x is nearest `target` in `x` (ascending), skipping points whose
/// y isn't finite (gaps). On a tie the earlier point wins. `None` when no point has a value or
/// `target` is `NaN`.
///
/// Binary search, then a walk outward past gaps.
pub fn nearest_x(x: &[f64], y: &[f64], target: f64) -> Option<usize> {
    if target.is_nan() {
        return None;
    }
    let n = x.len().min(y.len());
    let x = &x[..n];
    let split = x.partition_point(|v| *v < target);
    let right = (split..n).find(|&i| y[i].is_finite());
    let left = (0..split).rev().find(|&i| y[i].is_finite());
    match (left, right) {
        (Some(l), Some(r)) => Some(if target - x[l] <= x[r] - target { first_of(x, l) } else { r }),
        (Some(l), None) => Some(first_of(x, l)),
        (None, r) => r,
    }
}

/// The earliest index with the same x as `i`, among points that sit at that x.
fn first_of(x: &[f64], i: usize) -> usize {
    let mut i = i;
    while i > 0 && x[i - 1] == x[i] {
        i -= 1;
    }
    i
}

/// The index of the point drawn nearest `pointer`, within `radius` pixels, for scatter plots.
/// `to_px` maps a data point to pixels (usually two scales). Points that aren't finite are
/// skipped; on a tie the earlier point wins.
pub fn nearest_point(
    x: &[f64],
    y: &[f64],
    to_px: impl Fn(f64, f64) -> [f32; 2],
    pointer: [f32; 2],
    radius: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    let limit = radius * radius;
    for (i, (&px, &py)) in x.iter().zip(y).enumerate() {
        if !(px.is_finite() && py.is_finite()) {
            continue;
        }
        let p = to_px(px, py);
        let d = (p[0] - pointer[0]).powi(2) + (p[1] - pointer[1]).powi(2);
        if d <= limit && best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// The band (bar, category) under pixel `px` on `band`'s axis, counting the gaps around each
/// band as part of it so the hover doesn't flicker between bars.
pub fn band_at(band: &Band, px: f32) -> Option<usize> {
    band.index_at(px)
}

/// The heat-map cell `(column, row)` under `point`, with `columns` the x bands and `rows` the y
/// bands.
pub fn cell_at(columns: &Band, rows: &Band, point: [f32; 2]) -> Option<(usize, usize)> {
    Some((columns.index_at(point[0])?, rows.index_at(point[1])?))
}

/// The pie or donut slice under `point`, as the [`PieSlice::index`] of its value: the point must
/// be between `inner` and `outer` pixels from `center`, and its angle within the slice's angles
/// (padding included, so the gaps between slices don't flicker).
pub fn slice_at(slices: &[PieSlice], center: [f32; 2], inner: f32, outer: f32, point: [f32; 2]) -> Option<usize> {
    let r = (point[0] - center[0]).hypot(point[1] - center[1]);
    if !(r >= inner.min(outer) && r <= outer.max(inner)) {
        return None;
    }
    let a = angle_at(center, point);
    slices.iter().find(|s| contains_angle(s.start, s.end, a)).map(|s| s.index)
}

fn contains_angle(start: f64, end: f64, a: f64) -> bool {
    let span = (end - start).abs();
    if span.is_nan() || span <= 0.0 {
        return false;
    }
    if span >= TAU {
        return true;
    }
    let from = if end >= start { a - start } else { start - a };
    from.rem_euclid(TAU) < span
}

/// The radar axis nearest the direction of `point` from `center`, for `count` axes starting at
/// `start_angle` (see [`radar_angles`](crate::layout::radar_angles)). `None` without axes or at
/// the center itself.
pub fn radar_axis_at(count: usize, start_angle: f64, center: [f32; 2], point: [f32; 2]) -> Option<usize> {
    if count == 0 || point == center || !start_angle.is_finite() {
        return None;
    }
    let step = TAU / count as f64;
    let k = ((angle_at(center, point) - start_angle) / step).round();
    if !k.is_finite() {
        return None;
    }
    Some((k as i64).rem_euclid(count as i64) as usize)
}

#[cfg(test)]
#[path = "hit_tests.rs"]
mod tests;
