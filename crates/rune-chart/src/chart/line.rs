//! A line through a series, straight, smooth or stepped, with optional point markers.

use rune::{Color, LineCap, Shape};

use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::Series;
use crate::decimate::{decimate, visible_range};
use crate::hit::nearest_x;
use crate::layout::{Curve, split_gaps};

/// A line through `series` (x ascending, `NaN` y a gap).
#[derive(Clone)]
pub struct Line {
    name: String,
    series: Series,
    color: Option<Color>,
    width: f32,
    curve: Curve,
    dashed: Option<(f32, f32)>,
    markers: Option<f32>,
}

impl Line {
    pub fn new(name: impl Into<String>, series: Series) -> Line {
        Line { name: name.into(), series, color: None, width: 1.75, curve: Curve::Linear, dashed: None, markers: None }
    }

    pub fn color(mut self, color: Color) -> Line {
        self.color = Some(color);
        self
    }

    /// Stroke width in points.
    pub fn width(mut self, width: f32) -> Line {
        self.width = width;
        self
    }

    /// Straight (default), smooth (`Curve::MonotoneX`) or stepped (`Curve::Step(_)`).
    pub fn curve(mut self, curve: Curve) -> Line {
        self.curve = curve;
        self
    }

    /// Dashes of `dash` points with `gap` points between.
    pub fn dashed(mut self, dash: f32, gap: f32) -> Line {
        self.dashed = Some((dash, gap));
        self
    }

    /// A dot of `radius` points at each point (while they're at least a few points apart).
    pub fn markers(mut self, radius: f32) -> Line {
        self.markers = Some(radius);
        self
    }

    pub fn series(&self) -> &Series {
        &self.series
    }

    /// The visible points, decimated to the frame's pixel columns, in the frame's points
    /// (`NaN` for gaps).
    pub(crate) fn points(series: &Series, frame: &Frame) -> Vec<[f32; 2]> {
        let (x, y) = (series.x(), series.y());
        let range = visible_range(x, frame.viewport.x);
        decimate(x, y, range, frame.viewport.x, frame.columns())
            .into_iter()
            .map(|i| if y[i].is_finite() { frame.px(x[i], y[i]) } else { [f32::NAN, f32::NAN] })
            .collect()
    }

    pub(crate) fn hover_series(series: &Series, frame: &Frame, x: f64, name: &str, color: Color) -> Option<HoverPoint> {
        let i = nearest_x(series.x(), series.y(), x)?;
        let (px, py) = (series.x()[i], series.y()[i]);
        if px != x || !py.is_finite() {
            return None;
        }
        Some(HoverPoint {
            x: px,
            dot: Some(frame.px(px, py)),
            row: TooltipRow { color, label: name.to_owned(), value: (frame.format_y)(py) },
        })
    }
}

impl Plot for Line {
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
        self.series.x_extent()
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        self.series.y_extent_in(visible_range(self.series.x(), x))
    }

    fn draw(&self, frame: &Frame, mut shape: Shape) -> Shape {
        let color = self.color();
        let points = Line::points(&self.series, frame);
        for run in split_gaps(&points) {
            let line = self.curve.polyline(run, 0.25);
            shape = match self.dashed {
                Some((dash, gap)) => shape.stroke_dashed(line, self.width, dash, gap, LineCap::Butt, color),
                None => shape.stroke(line, self.width, color),
            };
        }
        if let Some(radius) = self.markers {
            let sparse = points.len() as f32 * radius * 3.0 < frame.size[0];
            for p in points.iter().filter(|p| sparse && p[0].is_finite()) {
                shape = shape.dot(*p, radius, color);
            }
        }
        shape
    }

    fn snap(&self, x: f64) -> Option<f64> {
        nearest_x(self.series.x(), self.series.y(), x).map(|i| self.series.x()[i])
    }

    fn hover(&self, frame: &Frame, x: f64, _: [f32; 2]) -> Option<HoverPoint> {
        Line::hover_series(&self.series, frame, x, &self.name, self.color())
    }
}
