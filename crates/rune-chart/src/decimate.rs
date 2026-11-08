//! Fewer points for the same picture: the visible part of a sorted column, and min/max
//! decimation per pixel column.
//!
//! A line of 50,000 points across 800 pixels draws the same as one that keeps, per pixel column,
//! the first, last, lowest and highest point (M4 decimation): spikes stay, unlike averaging,
//! and the line enters and leaves each column where it did.

use std::ops::Range;

/// The indices of `x` (ascending) to draw for the visible domain `(lo, hi)`: every point
/// inside, plus one beyond each edge so lines reach the plot's edges.
///
/// Empty when `x` is empty or the domain isn't finite.
pub fn visible_range(x: &[f64], domain: (f64, f64)) -> Range<usize> {
    let (lo, hi) = if domain.1 < domain.0 { (domain.1, domain.0) } else { domain };
    if !(lo.is_finite() && hi.is_finite()) {
        return 0..0;
    }
    let start = x.partition_point(|v| *v < lo).saturating_sub(1);
    let end = (x.partition_point(|v| *v <= hi) + 1).min(x.len());
    start..end.max(start)
}

/// The indices within `range` to draw for a line over `domain` on a plot `columns` pixels wide,
/// in order.
///
/// Per pixel column, keeps the first, lowest, highest and last point. A point whose y isn't
/// finite (a gap) is kept as it is and ends the column, so the line still breaks there. When
/// the range has fewer than two points per pixel column, every index is returned.
pub fn decimate(x: &[f64], y: &[f64], range: Range<usize>, domain: (f64, f64), columns: usize) -> Vec<usize> {
    let end = range.end.min(x.len()).min(y.len());
    let range = range.start.min(end)..end;
    let (lo, hi) = if domain.1 < domain.0 { (domain.1, domain.0) } else { domain };
    let span = hi - lo;
    if columns == 0 || range.len() < columns.saturating_mul(2) || !(span.is_finite() && span > 0.0) {
        return range.collect();
    }
    let scale = columns as f64 / span;
    let mut out = Vec::with_capacity(columns.saturating_mul(4).min(range.len()));
    let mut bucket: Option<Bucket> = None;
    for i in range {
        if !y[i].is_finite() {
            flush(&mut bucket, &mut out);
            out.push(i);
            continue;
        }
        let column = ((x[i] - lo) * scale).floor();
        match &mut bucket {
            Some(b) if b.column == column => b.add(i, y),
            _ => {
                flush(&mut bucket, &mut out);
                bucket = Some(Bucket { column, first: i, last: i, min: i, max: i });
            }
        }
    }
    flush(&mut bucket, &mut out);
    out
}

/// The points of one pixel column so far.
struct Bucket {
    column: f64,
    first: usize,
    last: usize,
    min: usize,
    max: usize,
}

impl Bucket {
    fn add(&mut self, i: usize, y: &[f64]) {
        self.last = i;
        if y[i] < y[self.min] {
            self.min = i;
        }
        if y[i] > y[self.max] {
            self.max = i;
        }
    }
}

fn flush(bucket: &mut Option<Bucket>, out: &mut Vec<usize>) {
    let Some(b) = bucket.take() else { return };
    let mut keep = [b.first, b.min, b.max, b.last];
    keep.sort_unstable();
    for (k, &i) in keep.iter().enumerate() {
        if k == 0 || keep[k - 1] != i {
            out.push(i);
        }
    }
}

#[cfg(test)]
#[path = "decimate_tests.rs"]
mod tests;
