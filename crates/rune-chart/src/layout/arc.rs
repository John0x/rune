//! Arc geometry for pie and donut slices: outlines as polygons, and label anchors.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::polar;

const EPSILON: f64 = 1e-9;
const MAX_SEGMENTS: usize = 1024;

/// The outline of a pie slice (`inner` 0) or donut segment from `start` to `end`, as a polygon
/// whose curved edges are flattened so they stray at most `tolerance` pixels from the true arc.
///
/// `pad` is the slice's padding angle (from [`PieSlice`](super::PieSlice)); as d3's arc, it
/// becomes a gap of even width between slices, measured at the radius `√(inner² + outer²)`.
/// A full turn gives a circle, or for a donut a ring cut open along one seam (outer circle, then
/// inner circle backward), which fills as a ring. Empty when `outer` isn't positive.
pub fn arc_polygon(
    center: [f32; 2],
    inner: f32,
    outer: f32,
    start: f64,
    end: f64,
    pad: f64,
    tolerance: f32,
) -> Vec<[f32; 2]> {
    let (r0, r1) = (f64::from(inner.min(outer).max(0.0)), f64::from(inner.max(outer)));
    if !(r1 > 0.0 && r1.is_finite() && start.is_finite() && end.is_finite()) {
        return Vec::new();
    }
    let tolerance = f64::from(tolerance);
    let mut out = Vec::new();
    let da = (end - start).abs();
    if da >= TAU - EPSILON {
        arc_points(&mut out, center, r1, start, start + TAU, tolerance);
        if r0 > EPSILON {
            arc_points(&mut out, center, r0, start + TAU, start, tolerance);
        }
        return out;
    }
    let cw = if end >= start { 1.0 } else { -1.0 };
    let (mut a00, mut a10, mut a01, mut a11) = (start, end, start, end);
    let rp = (r0 * r0 + r1 * r1).sqrt();
    let ap = if pad.is_finite() { pad.abs() / 2.0 } else { 0.0 };
    if ap > EPSILON {
        let inset = |r: f64| if r > EPSILON { (rp / r * ap.sin()).clamp(-1.0, 1.0).asin() } else { f64::INFINITY };
        let (p0, p1) = (inset(r0), inset(r1));
        if da - p0 * 2.0 > EPSILON {
            (a00, a10) = (a00 + p0 * cw, a10 - p0 * cw);
        } else {
            (a00, a10) = ((start + end) / 2.0, (start + end) / 2.0);
        }
        if da - p1 * 2.0 > EPSILON {
            (a01, a11) = (a01 + p1 * cw, a11 - p1 * cw);
        } else {
            (a01, a11) = ((start + end) / 2.0, (start + end) / 2.0);
        }
    }
    arc_points(&mut out, center, r1, a01, a11, tolerance);
    if r0 > EPSILON {
        arc_points(&mut out, center, r0, a10, a00, tolerance);
    } else {
        out.push(center);
    }
    out
}

/// Points along the arc at radius `r` from `a` to `b`, both ends included.
fn arc_points(out: &mut Vec<[f32; 2]>, center: [f32; 2], r: f64, a: f64, b: f64, tolerance: f64) {
    let span = (b - a).abs();
    let tolerance = if tolerance.is_finite() && tolerance > 0.0 { tolerance } else { 0.25 };
    let step = if tolerance >= r { FRAC_PI_2 } else { 2.0 * (1.0 - tolerance / r).acos() };
    let n = ((span / step).ceil() as usize).clamp(1, MAX_SEGMENTS);
    for i in 0..=n {
        out.push(polar(center, a + (b - a) * i as f64 / n as f64, r as f32));
    }
}

/// The middle of a slice: halfway between its radii and its angles, as d3's `arc.centroid`.
/// Where a label inside the slice goes.
pub fn arc_centroid(center: [f32; 2], inner: f32, outer: f32, start: f64, end: f64) -> [f32; 2] {
    polar(center, (start + end) / 2.0, (inner + outer) / 2.0)
}

/// Which side of the pie a label sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelSide {
    /// Left of the pie: right-align the text at the line's end.
    Left,
    /// Right of the pie: left-align the text at the line's end.
    Right,
}

/// A leader line from a slice's edge to its label outside the pie.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeaderLine {
    /// On the slice's outer edge at its middle angle, then `elbow` further out, then
    /// horizontally away from the pie. The label goes at the last point.
    pub points: [[f32; 2]; 3],
    /// The side the line runs out to.
    pub side: LabelSide,
}

/// The leader line for a slice of radius `outer`: out by `elbow` pixels along the middle angle,
/// then `length` pixels sideways.
pub fn leader_line(center: [f32; 2], outer: f32, start: f64, end: f64, elbow: f32, length: f32) -> LeaderLine {
    let mid = (start + end) / 2.0;
    let edge = polar(center, mid, outer);
    let bend = polar(center, mid, outer + elbow);
    let side = if mid.sin() < 0.0 { LabelSide::Left } else { LabelSide::Right };
    let dx = if side == LabelSide::Left { -length } else { length };
    LeaderLine { points: [edge, bend, [bend[0] + dx, bend[1]]], side }
}

#[cfg(test)]
#[path = "arc_tests.rs"]
mod tests;
