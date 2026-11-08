//! The visible part of a chart's data space, and the zoom, pan and fit operations on it.
//!
//! A [`Viewport`] holds the visible x and y domains; scales are built from it, not from the
//! whole data, so zooming is changing the viewport. Every operation returns a new viewport.
//!
//! Pixel arguments are relative to the plot area: `[0, 0]` is its top-left corner and `size` its
//! width and height. x grows to the right with the data; y grows downward on screen while data y
//! grows upward, as on a usual chart.

use crate::data::{Series, extent_in, union};

/// The visible x and y domains, each `(min, max)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// The visible x domain.
    pub x: (f64, f64),
    /// The visible y domain.
    pub y: (f64, f64),
}

/// Which axes an operation changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Axes {
    /// Only x (time series zoom and pan).
    X,
    /// Only y.
    Y,
    /// Both.
    #[default]
    Both,
}

impl Axes {
    fn x(self) -> bool {
        matches!(self, Axes::X | Axes::Both)
    }

    fn y(self) -> bool {
        matches!(self, Axes::Y | Axes::Both)
    }
}

/// How far a viewport may go: bounds per axis (usually the data's extent plus a margin) and the
/// smallest span per axis (so zooming in stops somewhere). See [`Viewport::clamp`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Limits {
    /// Bounds for x, or `None` for none.
    pub x: Option<(f64, f64)>,
    /// Bounds for y, or `None` for none.
    pub y: Option<(f64, f64)>,
    /// The smallest x and y spans; 0 for no minimum.
    pub min_span: (f64, f64),
}

impl Limits {
    /// Bounds around `extent`, widened on each side by `margin` times its span: `around(data,
    /// 0.1)` lets the view go a tenth past the data.
    pub fn around(extent: Viewport, margin: f64) -> Limits {
        let pad = |(a, b): (f64, f64)| {
            let (a, b) = ordered((a, b));
            let m = (b - a) * margin.max(0.0);
            (a - m, b + m)
        };
        Limits { x: Some(pad(extent.x)), y: Some(pad(extent.y)), min_span: (0.0, 0.0) }
    }

    /// The same limits with the smallest x and y spans.
    pub fn min_span(self, x: f64, y: f64) -> Limits {
        Limits { min_span: (x, y), ..self }
    }
}

impl Viewport {
    /// A viewport over the given domains.
    pub const fn new(x: (f64, f64), y: (f64, f64)) -> Viewport {
        Viewport { x, y }
    }

    /// A viewport around data extents, each side padded by `padding` times the span
    /// (`(0.0, 0.05)` is flush in x and 5% above and below in y). An extent with no span (a
    /// single point, a flat line) is widened around its value so the data stays visible.
    pub fn fit(x: (f64, f64), y: (f64, f64), padding: (f64, f64)) -> Viewport {
        Viewport { x: pad(x, padding.0), y: pad(y, padding.1) }
    }

    /// A viewport around every point of `series`, padded as in [`Viewport::fit`], or `None` when
    /// there is no point with a value.
    pub fn fit_series<'a>(series: impl IntoIterator<Item = &'a Series>, padding: (f64, f64)) -> Option<Viewport> {
        let (mut x, mut y) = (None, None);
        for s in series {
            if let Some(sy) = s.y_extent() {
                y = union(y, Some(sy));
                x = union(x, s.x_extent());
            }
        }
        Some(Viewport::fit(x?, y?, padding))
    }

    /// The width of the x domain.
    pub fn span_x(&self) -> f64 {
        self.x.1 - self.x.0
    }

    /// The height of the y domain.
    pub fn span_y(&self) -> f64 {
        self.y.1 - self.y.0
    }

    /// The data point under pixel `px` of a plot of `size`.
    pub fn point_at(&self, px: [f32; 2], size: [f32; 2]) -> (f64, f64) {
        let fx = if size[0] > 0.0 { f64::from(px[0]) / f64::from(size[0]) } else { 0.5 };
        let fy = if size[1] > 0.0 { f64::from(px[1]) / f64::from(size[1]) } else { 0.5 };
        (self.x.0 + self.span_x() * fx, self.y.1 - self.span_y() * fy)
    }

    /// Zooms about `anchor` (a data point, usually the one under the pointer, which stays under
    /// it) by `factor` on `axes`: 2 shows half the span, 0.5 twice the span. A factor that isn't
    /// positive and finite leaves the viewport as it is.
    pub fn zoom(self, anchor: (f64, f64), factor: f64, axes: Axes) -> Viewport {
        if !(factor.is_finite() && factor > 0.0) {
            return self;
        }
        let z = |(a, b): (f64, f64), at: f64| {
            if !at.is_finite() {
                return (a, b);
            }
            (at - (at - a) / factor, at + (b - at) / factor)
        };
        Viewport {
            x: if axes.x() { z(self.x, anchor.0) } else { self.x },
            y: if axes.y() { z(self.y, anchor.1) } else { self.y },
        }
    }

    /// Moves the view with a drag of `delta` pixels on a plot of `size`, so the data follows the
    /// pointer: dragging right shows earlier x, dragging down shows higher y.
    pub fn pan(self, delta: [f32; 2], size: [f32; 2], axes: Axes) -> Viewport {
        let mut v = self;
        if axes.x() && size[0] > 0.0 && delta[0].is_finite() {
            let d = -f64::from(delta[0]) / f64::from(size[0]) * self.span_x();
            v.x = (self.x.0 + d, self.x.1 + d);
        }
        if axes.y() && size[1] > 0.0 && delta[1].is_finite() {
            let d = f64::from(delta[1]) / f64::from(size[1]) * self.span_y();
            v.y = (self.y.0 + d, self.y.1 + d);
        }
        v
    }

    /// Zooms to the pixel rectangle between corners `a` and `b` (in any order) on a plot of
    /// `size`. An axis the rectangle is under a pixel thick on, or one not in `axes`, keeps its
    /// domain.
    pub fn box_zoom(self, a: [f32; 2], b: [f32; 2], size: [f32; 2], axes: Axes) -> Viewport {
        let (p, q) = (self.point_at(a, size), self.point_at(b, size));
        let mut v = self;
        if axes.x() && (a[0] - b[0]).abs() >= 1.0 {
            v.x = ordered((p.0, q.0));
        }
        if axes.y() && (a[1] - b[1]).abs() >= 1.0 {
            v.y = ordered((p.1, q.1));
        }
        v
    }

    /// The viewport kept within `limits`: each span grown to at least the minimum span and
    /// shrunk to at most the bounds, then shifted inside the bounds. A minimum span larger than
    /// the bounds wins, centered on them.
    pub fn clamp(self, limits: &Limits) -> Viewport {
        Viewport {
            x: clamp_axis(self.x, limits.x, limits.min_span.0),
            y: clamp_axis(self.y, limits.y, limits.min_span.1),
        }
    }

    /// The viewport with y fitted to the points of `series` inside the visible x range, padded
    /// by `padding` times the span on each side, so y follows what is on screen while x zooms
    /// and pans. Unchanged when no point with a value is visible.
    pub fn fit_y_to_visible<'a>(self, series: impl IntoIterator<Item = &'a Series>, padding: f64) -> Viewport {
        let (lo, hi) = ordered(self.x);
        let mut y = None;
        for s in series {
            let x = s.x();
            let range = x.partition_point(|v| *v < lo)..x.partition_point(|v| *v <= hi);
            y = union(y, extent_in(s.y(), range));
        }
        match y {
            Some(y) => Viewport { y: pad(y, padding), ..self },
            None => self,
        }
    }

    /// The viewport `t` of the way from `self` to `to` (0 is `self`, 1 is `to`), for animating
    /// between viewports.
    pub fn lerp(self, to: Viewport, t: f64) -> Viewport {
        let l = |a: f64, b: f64| a + (b - a) * t;
        Viewport { x: (l(self.x.0, to.x.0), l(self.x.1, to.x.1)), y: (l(self.y.0, to.y.0), l(self.y.1, to.y.1)) }
    }

    /// Whether `other` differs by at most `fraction` of this viewport's span on every bound, so
    /// an animation toward it can stop: `approx_eq(target, 1e-3)` is within a thousandth.
    pub fn approx_eq(&self, other: &Viewport, fraction: f64) -> bool {
        let close = |a: f64, b: f64, span: f64| (a - b).abs() <= fraction * span.abs().max(f64::MIN_POSITIVE);
        close(self.x.0, other.x.0, self.span_x())
            && close(self.x.1, other.x.1, self.span_x())
            && close(self.y.0, other.y.0, self.span_y())
            && close(self.y.1, other.y.1, self.span_y())
    }
}

fn ordered((a, b): (f64, f64)) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

/// Pads an extent by `fraction` of its span on each side, widening a zero span around its value
/// (by a tenth of it, or 1 around zero).
fn pad(extent: (f64, f64), fraction: f64) -> (f64, f64) {
    let (a, b) = ordered(extent);
    let span = b - a;
    if span > 0.0 {
        let p = span * fraction.max(0.0);
        return (a - p, b + p);
    }
    let half = if a == 0.0 { 1.0 } else { a.abs() * 0.1 };
    (a - half, b + half)
}

fn clamp_axis(range: (f64, f64), bounds: Option<(f64, f64)>, min_span: f64) -> (f64, f64) {
    let (mut a, mut b) = ordered(range);
    if !(a.is_finite() && b.is_finite()) {
        return bounds.map(ordered).unwrap_or(range);
    }
    let min_span = if min_span.is_finite() { min_span.max(0.0) } else { 0.0 };
    if b - a < min_span {
        let c = (a + b) / 2.0;
        (a, b) = (c - min_span / 2.0, c + min_span / 2.0);
    }
    let Some((lo, hi)) = bounds.map(ordered).filter(|(lo, hi)| lo.is_finite() && hi.is_finite()) else {
        return (a, b);
    };
    let span = b - a;
    if span >= hi - lo {
        let span = span.min(min_span.max(hi - lo));
        let c = (lo + hi) / 2.0;
        return (c - span / 2.0, c + span / 2.0);
    }
    if a < lo {
        (lo, lo + span)
    } else if b > hi {
        (hi - span, hi)
    } else {
        (a, b)
    }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
