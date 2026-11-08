//! A filled area: under a series down to a baseline, or a band between two series.

use rune::{Color, Gradient, Shape};

use super::geometry::{clamp_y, split_at};
use super::line::Line;
use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::{Series, union};
use crate::decimate::visible_range;
use crate::hit::nearest_x;
use crate::layout::{Curve, split_gaps};

/// An area under a series down to a baseline ([`Area::new`]), or a band between two series
/// ([`Area::band`]), with its edge stroked on top.
///
/// ```
/// use rune_chart::{Area, data::Series};
/// let price = Series::from_y([3.0, 4.0, 3.5, 5.0]);
/// let area = Area::new("Price", price).gradient(true);
/// ```
#[derive(Clone)]
pub struct Area {
    name: String,
    upper: Series,
    /// The band's lower series; `None` fills to `baseline`.
    lower: Option<Series>,
    baseline: f64,
    color: Option<Color>,
    curve: Curve,
    gradient: bool,
    line: bool,
    width: f32,
    opacity: f32,
}

impl Area {
    /// The area under `series` (x ascending, `NaN` y a gap) down to the baseline (0, see
    /// [`Area::baseline`]).
    pub fn new(name: impl Into<String>, series: Series) -> Area {
        Area {
            name: name.into(),
            upper: series,
            lower: None,
            baseline: 0.0,
            color: None,
            curve: Curve::Linear,
            gradient: false,
            line: true,
            width: 1.5,
            opacity: 0.3,
        }
    }

    /// The band from `lower` up to `upper` (a 25th percentile to the median). With equal x
    /// columns, a gap in either is a gap in the band; otherwise each is drawn at its own points.
    /// Its edges aren't stroked unless [`Area::line`] asks.
    pub fn band(name: impl Into<String>, upper: Series, lower: Series) -> Area {
        let (upper, lower) = mask_gaps(upper, lower);
        Area { lower: Some(lower), line: false, ..Area::new(name, upper) }
    }

    /// The y the area fills down (or up) to; 0 by default. The fill stops at the plot's edge
    /// when the baseline is out of view, and the baseline doesn't widen the fitted y domain.
    pub fn baseline(mut self, y: f64) -> Area {
        self.baseline = y;
        self
    }

    pub fn color(mut self, color: Color) -> Area {
        self.color = Some(color);
        self
    }

    /// Straight (default), smooth (`Curve::MonotoneX`) or stepped (`Curve::Step(_)`) edges.
    pub fn curve(mut self, curve: Curve) -> Area {
        self.curve = curve;
        self
    }

    /// Fades the fill from its full opacity at the line to nothing at the baseline (a band
    /// fades from its top toward its bottom).
    pub fn gradient(mut self, on: bool) -> Area {
        self.gradient = on;
        self
    }

    /// Whether to stroke the top edge (a band's two edges); on by default, off for a band.
    pub fn line(mut self, on: bool) -> Area {
        self.line = on;
        self
    }

    /// The edge's stroke width in points.
    pub fn width(mut self, width: f32) -> Area {
        self.width = width;
        self
    }

    /// The fill's opacity, 0 to 1 (0.3 by default).
    pub fn opacity(mut self, opacity: f32) -> Area {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// The area's top edge, or the band's upper series.
    pub fn series(&self) -> &Series {
        &self.upper
    }

    fn fill(&self) -> Color {
        let color = self.color();
        color.alpha(color.a * self.opacity)
    }

    fn draw_to_baseline(&self, frame: &Frame, mut shape: Shape, points: &[[f32; 2]]) -> Shape {
        let (color, fill) = (self.color(), self.fill());
        let base = clamp_y(frame.y.map(self.baseline), frame.size[1]);
        let ys = || points.iter().map(|p| p[1]).filter(|y| y.is_finite());
        let (top, bottom) = (ys().fold(base, f32::min), ys().fold(base, f32::max));
        for run in split_gaps(points) {
            let line = self.curve.polyline(run, 0.25);
            for (above, part) in split_at(&line, base) {
                let (Some(first), Some(last)) = (part.first(), part.last()) else { continue };
                let flat = vec![[first[0], base], [last[0], base]];
                let (upper, lower, gradient) = if above {
                    (part, flat, Gradient::vertical(top, fill, base, fill.alpha(0.0)))
                } else {
                    (flat, part, Gradient::vertical(base, fill.alpha(0.0), bottom, fill))
                };
                shape = if self.gradient {
                    shape.area_gradient(upper, lower, gradient)
                } else {
                    shape.area(upper, lower, fill)
                };
            }
            if self.line {
                shape = shape.stroke(line, self.width, color);
            }
        }
        shape
    }

    fn draw_band(&self, frame: &Frame, mut shape: Shape, points: &[[f32; 2]], lower: &Series) -> Shape {
        let (color, fill) = (self.color(), self.fill());
        let lower = Line::points(lower, frame);
        let top = points.iter().map(|p| p[1]).filter(|y| y.is_finite()).fold(f32::INFINITY, f32::min);
        let bottom = lower.iter().map(|p| p[1]).filter(|y| y.is_finite()).fold(f32::NEG_INFINITY, f32::max);
        for run in split_gaps(points) {
            let (a, b) = (run[0][0], run[run.len() - 1][0]);
            let within = |p: &&[f32; 2]| p[1].is_finite() && p[0] >= a - 0.01 && p[0] <= b + 0.01;
            let below: Vec<[f32; 2]> = lower.iter().filter(within).copied().collect();
            if run.len() < 2 || below.len() < 2 {
                continue;
            }
            let (upper, below) = (self.curve.polyline(run, 0.25), self.curve.polyline(&below, 0.25));
            shape = if self.gradient && top < bottom {
                let gradient = Gradient::vertical(top, fill, bottom, fill.alpha(fill.a * 0.2));
                shape.area_gradient(upper.clone(), below.clone(), gradient)
            } else {
                shape.area(upper.clone(), below.clone(), fill)
            };
            if self.line {
                shape = shape.stroke(upper, self.width, color).stroke(below, self.width, color);
            }
        }
        shape
    }
}

/// With equal x columns, `upper` and `lower` with a gap wherever either has one.
fn mask_gaps(upper: Series, lower: Series) -> (Series, Series) {
    if upper.x() != lower.x() || upper.y().len() != lower.y().len() {
        return (upper, lower);
    }
    let mask = |s: &Series, other: &Series| {
        let y: Vec<f64> =
            s.y().iter().zip(other.y().iter()).map(|(a, b)| if b.is_finite() { *a } else { f64::NAN }).collect();
        Series::new(s.x().clone(), y).unwrap_or_else(|_| s.clone())
    };
    (mask(&upper, &lower), mask(&lower, &upper))
}

impl Plot for Area {
    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> Color {
        self.color.unwrap_or(Color::WHITE)
    }

    fn set_default_color(&mut self, color: Color) {
        self.color.get_or_insert(color);
    }

    fn x_extent(&self) -> Option<(f64, f64)> {
        union(self.upper.x_extent(), self.lower.as_ref().and_then(Series::x_extent))
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        let extent = |s: &Series| s.y_extent_in(visible_range(s.x(), x));
        union(extent(&self.upper), self.lower.as_ref().and_then(extent))
    }

    fn draw(&self, frame: &Frame, shape: Shape) -> Shape {
        let points = Line::points(&self.upper, frame);
        match &self.lower {
            Some(lower) => self.draw_band(frame, shape, &points, lower),
            None => self.draw_to_baseline(frame, shape, &points),
        }
    }

    fn snap(&self, x: f64) -> Option<f64> {
        nearest_x(self.upper.x(), self.upper.y(), x).map(|i| self.upper.x()[i])
    }

    fn hover(&self, frame: &Frame, x: f64, _: [f32; 2]) -> Option<HoverPoint> {
        let mut point = Line::hover_series(&self.upper, frame, x, &self.name, self.color())?;
        if let Some(lower) = &self.lower {
            let i = nearest_x(lower.x(), lower.y(), x).filter(|&i| lower.x()[i] == x)?;
            let value = format!("{} – {}", (frame.format_y)(lower.y()[i]), point.row.value);
            point.row = TooltipRow { value, ..point.row };
        }
        Some(point)
    }
}
