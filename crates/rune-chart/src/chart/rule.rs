//! Reference marks: a horizontal or vertical line at a value, or a shaded horizontal band.

use std::collections::BTreeSet;

use rune::{Color, LineCap, Shape};

use super::plot::{Frame, HoverPoint, Plot, PlotLabel};
use crate::style::ChartStyle;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    Y(f64),
    X(f64),
    Band(f64, f64),
}

/// A reference mark: a horizontal line at a y ([`Rule::y`]), a vertical one at an x
/// ([`Rule::x`]) or a shaded horizontal band ([`Rule::y_band`]), labelled with its name inside
/// the plot. It has no hover and doesn't widen the fitted x domain; y marks are kept in view
/// unless [`Rule::fit`] says otherwise. Not in the legend unless [`Rule::in_legend`] asks.
///
/// ```
/// use rune_chart::Rule;
/// let median = Rule::y("7-day median", 19.2).dashed(4.0, 4.0);
/// let target = Rule::y_band("Target", 18.0, 21.0);
/// ```
#[derive(Clone)]
pub struct Rule {
    name: String,
    mark: Mark,
    color: Option<Color>,
    width: f32,
    dashed: Option<(f32, f32)>,
    label: bool,
    legend: bool,
    fit: bool,
}

impl Rule {
    fn of(name: impl Into<String>, mark: Mark) -> Rule {
        let fit = !matches!(mark, Mark::X(_));
        Rule { name: name.into(), mark, color: None, width: 1.0, dashed: None, label: true, legend: false, fit }
    }

    /// A horizontal line across the plot at `value`.
    pub fn y(name: impl Into<String>, value: f64) -> Rule {
        Rule::of(name, Mark::Y(value))
    }

    /// A vertical line down the plot at data x `value` (a release, an event).
    pub fn x(name: impl Into<String>, value: f64) -> Rule {
        Rule::of(name, Mark::X(value))
    }

    /// A shaded band across the plot from `from` to `to` (a target range).
    pub fn y_band(name: impl Into<String>, from: f64, to: f64) -> Rule {
        Rule::of(name, Mark::Band(from.min(to), from.max(to)))
    }

    /// Its colour; the style's crosshair colour by default (a band shades in it faintly).
    pub fn color(mut self, color: Color) -> Rule {
        self.color = Some(color);
        self
    }

    /// The line's width in points (1 by default).
    pub fn width(mut self, width: f32) -> Rule {
        self.width = width;
        self
    }

    /// Dashes of `dash` points with `gap` points between.
    pub fn dashed(mut self, dash: f32, gap: f32) -> Rule {
        self.dashed = Some((dash, gap));
        self
    }

    /// Whether to label it with its name inside the plot (on by default).
    pub fn label(mut self, on: bool) -> Rule {
        self.label = on;
        self
    }

    /// Whether it gets a legend entry, which can hide it (off by default).
    pub fn in_legend(mut self, on: bool) -> Rule {
        self.legend = on;
        self
    }

    /// Whether the fitted y domain keeps it in view (on by default for y lines and bands; an x
    /// line never changes y).
    pub fn fit(mut self, on: bool) -> Rule {
        self.fit = on;
        self
    }

    fn line(&self, shape: Shape, a: [f32; 2], b: [f32; 2]) -> Shape {
        let color = self.color();
        match self.dashed {
            Some((dash, gap)) => shape.stroke_dashed(vec![a, b], self.width, dash, gap, LineCap::Butt, color),
            None => shape.line(a, b, self.width, LineCap::Butt, color),
        }
    }
}

/// Snaps a line's position to the pixel grid, so a 1-point rule is crisp.
fn crisp(v: f32) -> f32 {
    v.round() + 0.5
}

impl Plot for Rule {
    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> Color {
        self.color.unwrap_or(Color::WHITE)
    }

    /// Rules don't take the palette: their colour comes from the style in `prepare`.
    fn set_default_color(&mut self, _: Color) {}

    fn prepare(&mut self, style: &ChartStyle, _: usize, _: &BTreeSet<String>) {
        self.color.get_or_insert(style.crosshair);
    }

    fn x_extent(&self) -> Option<(f64, f64)> {
        None
    }

    fn y_extent(&self, _: (f64, f64)) -> Option<(f64, f64)> {
        let extent = match self.mark {
            Mark::Y(v) => (v, v),
            Mark::Band(a, b) => (a, b),
            Mark::X(_) => return None,
        };
        (self.fit && extent.0.is_finite() && extent.1.is_finite()).then_some(extent)
    }

    fn draw(&self, frame: &Frame, shape: Shape) -> Shape {
        let [w, h] = frame.size;
        match self.mark {
            Mark::Y(v) => {
                let y = crisp(frame.y.map(v));
                if !y.is_finite() {
                    return shape;
                }
                self.line(shape, [0.0, y], [w, y])
            }
            Mark::X(v) => {
                let x = crisp(frame.x.map(v));
                if !x.is_finite() {
                    return shape;
                }
                self.line(shape, [x, 0.0], [x, h])
            }
            Mark::Band(a, b) => {
                let (top, bottom) = (frame.y.map(b).clamp(0.0, h), frame.y.map(a).clamp(0.0, h));
                if !(top.is_finite() && bottom.is_finite()) || bottom - top <= 0.0 {
                    return shape;
                }
                let color = self.color();
                shape.fill(vec![[0.0, top], [w, top], [w, bottom], [0.0, bottom]], color.alpha(color.a * 0.12))
            }
        }
    }

    fn hover(&self, _: &Frame, _: f64, _: [f32; 2]) -> Option<HoverPoint> {
        None
    }

    fn in_legend(&self) -> bool {
        self.legend
    }

    fn labels(&self, frame: &Frame) -> Vec<PlotLabel> {
        if !self.label || self.name.is_empty() {
            return Vec::new();
        }
        let [w, h] = frame.size;
        // Just above a y line at the right edge; inside a band's top at the left edge (clear of
        // y lines' labels); left of an x line at the top.
        let (at, anchor) = match self.mark {
            Mark::Y(v) => ([w - 4.0, frame.y.map(v) - 2.0], [1.0, 1.0]),
            Mark::Band(_, b) => ([4.0, frame.y.map(b).clamp(0.0, h) + 2.0], [0.0, 0.0]),
            Mark::X(v) => ([frame.x.map(v) - 4.0, 4.0], [1.0, 0.0]),
        };
        let visible = match self.mark {
            Mark::X(v) => frame.x_visible(v),
            _ => at[1].is_finite() && at[1] >= -20.0 && at[1] <= h + 20.0,
        };
        if !visible {
            return Vec::new();
        }
        vec![PlotLabel { text: self.name.clone(), at, anchor, color: self.color() }]
    }
}
