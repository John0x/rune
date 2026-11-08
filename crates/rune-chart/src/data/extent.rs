//! Minimum and maximum of a column.

use std::ops::Range;

/// The smallest and largest finite value, or `None` when there is none (empty, or all `NaN`).
pub fn extent(values: &[f64]) -> Option<(f64, f64)> {
    values.iter().copied().filter(|v| v.is_finite()).fold(None, |acc, v| match acc {
        None => Some((v, v)),
        Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
    })
}

/// [`extent`] over `values[range]`, with the range cut to the slice.
pub fn extent_in(values: &[f64], range: Range<usize>) -> Option<(f64, f64)> {
    let end = range.end.min(values.len());
    let start = range.start.min(end);
    extent(&values[start..end])
}

/// The extent covering both, either of which may be missing.
pub fn union(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
        (a, None) => a,
        (None, b) => b,
    }
}
