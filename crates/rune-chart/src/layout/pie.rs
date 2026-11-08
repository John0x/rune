//! Pie and donut angles, as d3's `pie`.

use std::f64::consts::TAU;

/// The order slices go around the circle in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PieSort {
    /// In the order of the values (the default, so slices match a legend).
    #[default]
    None,
    /// Largest first.
    Descending,
    /// Smallest first.
    Ascending,
}

/// How [`pie`] lays out its slices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieOptions {
    /// Where the first slice starts: 0 is twelve o'clock.
    pub start_angle: f64,
    /// Where the last slice ends; `start_angle + 2π` for a full pie. Below `start_angle` the
    /// slices go counterclockwise.
    pub end_angle: f64,
    /// The angle left empty between neighbouring slices.
    pub pad_angle: f64,
    /// The order of the slices around the circle.
    pub sort: PieSort,
}

impl Default for PieOptions {
    fn default() -> PieOptions {
        PieOptions { start_angle: 0.0, end_angle: TAU, pad_angle: 0.0, sort: PieSort::None }
    }
}

/// One slice of a pie: the angles it spans, padding included. Draw it from
/// `start + pad / 2` to `end - pad / 2`, or hand all three to
/// [`arc_polygon`](super::arc_polygon), which pads like d3 (gaps of even width).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieSlice {
    /// The index of the slice's value in the input.
    pub index: usize,
    /// The value, with negative and non-finite values counted as 0.
    pub value: f64,
    /// The start angle.
    pub start: f64,
    /// The end angle.
    pub end: f64,
    /// The padding angle to leave empty, split between the two ends.
    pub pad: f64,
}

/// One slice per value, in input order, each spanning an angle in proportion to its value.
///
/// Negative and non-finite values count as 0 (their slices are as wide as their padding). The
/// angle covered is at most a full turn; the padding is at most an equal share of it.
pub fn pie(values: &[f64], options: &PieOptions) -> Vec<PieSlice> {
    let values: Vec<f64> = values.iter().map(|v| if v.is_finite() && *v > 0.0 { *v } else { 0.0 }).collect();
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    let finite = |v: f64, or: f64| if v.is_finite() { v } else { or };
    let start = finite(options.start_angle, 0.0);
    let total = (finite(options.end_angle, start + TAU) - start).clamp(-TAU, TAU);
    let pad = finite(options.pad_angle, 0.0).abs().min(total.abs() / n as f64);
    let signed_pad = pad * total.signum();
    let sum: f64 = values.iter().sum();
    let k = if sum > 0.0 { (total - n as f64 * signed_pad) / sum } else { 0.0 };

    let mut order: Vec<usize> = (0..n).collect();
    match options.sort {
        PieSort::None => {}
        PieSort::Descending => order.sort_by(|&a, &b| values[b].total_cmp(&values[a])),
        PieSort::Ascending => order.sort_by(|&a, &b| values[a].total_cmp(&values[b])),
    }
    let mut slices = vec![PieSlice { index: 0, value: 0.0, start: 0.0, end: 0.0, pad: 0.0 }; n];
    let mut a0 = start;
    for i in order {
        let a1 = a0 + values[i] * k + signed_pad;
        slices[i] = PieSlice { index: i, value: values[i], start: a0, end: a1, pad };
        a0 = a1;
    }
    slices
}

#[cfg(test)]
#[path = "pie_tests.rs"]
mod tests;
