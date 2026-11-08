//! Curves through points in pixel space: straight, monotone cubic (as d3's `curveMonotoneX`)
//! and steps, as cubic Bézier segments or flattened polylines.
//!
//! Curves take runs of finite points; split a line at its gaps first with [`split_gaps`].

const MAX_SEGMENTS: usize = 256;

/// A cubic Bézier segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cubic {
    /// The start point.
    pub from: [f32; 2],
    /// The first control point.
    pub c1: [f32; 2],
    /// The second control point.
    pub c2: [f32; 2],
    /// The end point.
    pub to: [f32; 2],
}

impl Cubic {
    /// A straight segment, as a cubic with its controls at the thirds (so it flattens to one
    /// segment).
    pub fn line(from: [f32; 2], to: [f32; 2]) -> Cubic {
        let third = |k: f32| [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k];
        Cubic { from, c1: third(1.0 / 3.0), c2: third(2.0 / 3.0), to }
    }

    /// The point at `t` (0 is `from`, 1 is `to`).
    pub fn at(&self, t: f32) -> [f32; 2] {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        let p = |k: usize| a * self.from[k] + b * self.c1[k] + c * self.c2[k] + d * self.to[k];
        [p(0), p(1)]
    }

    /// Appends points along the curve, without `from`, so that no point of the curve lies more
    /// than about `tolerance` pixels from the polyline.
    pub fn flatten_into(&self, out: &mut Vec<[f32; 2]>, tolerance: f32) {
        let dd = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (a[0] - 2.0 * b[0] + c[0]).hypot(a[1] - 2.0 * b[1] + c[1]);
        let m = dd(self.from, self.c1, self.c2).max(dd(self.c1, self.c2, self.to));
        let tolerance = if tolerance.is_finite() && tolerance > 0.0 { tolerance } else { 0.25 };
        // Wang's formula for a cubic: n = √(3·m / (4·tolerance)).
        let n = (0.75 * m / tolerance).sqrt().ceil();
        let n = if n.is_finite() { (n as usize).clamp(1, MAX_SEGMENTS) } else { 1 };
        for i in 1..=n {
            out.push(self.at(i as f32 / n as f32));
        }
    }
}

/// Flattens consecutive cubics into one polyline from the first `from` (see
/// [`Cubic::flatten_into`]).
pub fn flatten(curves: &[Cubic], tolerance: f32) -> Vec<[f32; 2]> {
    let Some(first) = curves.first() else { return Vec::new() };
    let mut out = vec![first.from];
    for c in curves {
        c.flatten_into(&mut out, tolerance);
    }
    out
}

/// The runs of finite points between gaps (points with a `NaN` or infinite coordinate), each
/// drawn as its own line.
pub fn split_gaps(points: &[[f32; 2]]) -> impl Iterator<Item = &[[f32; 2]]> {
    points.split(|p| !(p[0].is_finite() && p[1].is_finite())).filter(|run| !run.is_empty())
}

/// A smooth curve through points with ascending x that never overshoots: between two points it
/// stays within their y values, so a monotone run stays monotone (Fritsch–Carlson, as d3's
/// `curveMonotoneX`). One cubic per pair of neighbouring points; consecutive duplicate points
/// are skipped. Empty for fewer than two distinct points.
pub fn monotone_x(points: &[[f32; 2]]) -> Vec<Cubic> {
    let mut p: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for q in points {
        let q = (f64::from(q[0]), f64::from(q[1]));
        if p.last() != Some(&q) {
            p.push(q);
        }
    }
    let n = p.len();
    if n < 2 {
        return Vec::new();
    }
    if n == 2 {
        return vec![Cubic::line(pt(p[0]), pt(p[1]))];
    }
    // Tangents at the inner points, then at the two ends.
    let mut t = vec![0.0; n];
    for i in 1..n - 1 {
        t[i] = slope3(p[i - 1], p[i], p[i + 1]);
    }
    t[0] = slope2(p[0], p[1], t[1]);
    t[n - 1] = slope2(p[n - 2], p[n - 1], t[n - 2]);
    (0..n - 1).map(|i| segment(p[i], p[i + 1], t[i], t[i + 1])).collect()
}

fn pt(p: (f64, f64)) -> [f32; 2] {
    [p.0 as f32, p.1 as f32]
}

fn sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The tangent at `b` from its neighbours, limited so the curve doesn't overshoot (Steffen's
/// method, as d3).
fn slope3(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let (h0, h1) = (b.0 - a.0, c.0 - b.0);
    let zero = |other: f64| if other < 0.0 { -0.0 } else { 0.0 };
    let s0 = (b.1 - a.1) / if h0 != 0.0 { h0 } else { zero(h1) };
    let s1 = (c.1 - b.1) / if h1 != 0.0 { h1 } else { zero(h0) };
    let p = (s0 * h1 + s1 * h0) / (h0 + h1);
    let t = (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs());
    if t.is_nan() { 0.0 } else { t }
}

/// The tangent at an end of the segment `a`–`b`, from the tangent `t` at its other end.
fn slope2(a: (f64, f64), b: (f64, f64), t: f64) -> f64 {
    let h = b.0 - a.0;
    if h != 0.0 { (3.0 * (b.1 - a.1) / h - t) / 2.0 } else { t }
}

/// The Hermite segment from `a` to `b` with tangents `t0` and `t1`, as a cubic.
fn segment(a: (f64, f64), b: (f64, f64), t0: f64, t1: f64) -> Cubic {
    let dx = (b.0 - a.0) / 3.0;
    Cubic { from: pt(a), c1: pt((a.0 + dx, a.1 + dx * t0)), c2: pt((b.0 - dx, b.1 - dx * t1)), to: pt(b) }
}

/// Where a step changes value between two points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StepPosition {
    /// At the earlier point: up or down first, then across (d3's `curveStepBefore`).
    Before,
    /// At the later point: across first, holding each value until the next (d3's
    /// `curveStepAfter`), as a running balance.
    #[default]
    After,
    /// Halfway between the points (d3's `curveStep`).
    Middle,
}

/// The polyline of a step line through `points`.
pub fn step(points: &[[f32; 2]], position: StepPosition) -> Vec<[f32; 2]> {
    let Some(&first) = points.first() else { return Vec::new() };
    let mut out = Vec::with_capacity(points.len() * 3);
    out.push(first);
    for w in points.windows(2) {
        let ([x0, y0], [x1, y1]) = (w[0], w[1]);
        match position {
            StepPosition::Before => out.push([x0, y1]),
            StepPosition::After => out.push([x1, y0]),
            StepPosition::Middle => {
                let mid = (x0 + x1) / 2.0;
                out.push([mid, y0]);
                out.push([mid, y1]);
            }
        }
        out.push([x1, y1]);
    }
    out
}

/// How a line or area connects its points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Curve {
    /// Straight segments.
    #[default]
    Linear,
    /// A monotone cubic (see [`monotone_x`]).
    MonotoneX,
    /// Steps (see [`step`]).
    Step(StepPosition),
}

impl Curve {
    /// The polyline to draw through `points` (finite, x ascending for `MonotoneX`), with curves
    /// flattened to `tolerance` pixels.
    pub fn polyline(self, points: &[[f32; 2]], tolerance: f32) -> Vec<[f32; 2]> {
        match self {
            Curve::Linear => points.to_vec(),
            Curve::Step(position) => step(points, position),
            Curve::MonotoneX => {
                let curves = monotone_x(points);
                if curves.is_empty() {
                    points.first().map(|p| vec![*p]).unwrap_or_default()
                } else {
                    flatten(&curves, tolerance)
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
