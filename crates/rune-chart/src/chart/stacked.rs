//! Areas stacked on each other: series sharing x, each layer resting on the ones below.

use std::collections::BTreeSet;
use std::sync::Arc;

use rune::{Color, Shape};

use super::geometry::{merge_indices, stack_at};
use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::Series;
use crate::decimate::{decimate, visible_range};
use crate::hit::nearest_x;
use crate::layout::{Curve, StackOffset};
use crate::style::ChartStyle;

/// A run of a layer between gaps: its top and bottom polylines in points.
type Run = (Vec<[f32; 2]>, Vec<[f32; 2]>);

#[derive(Clone)]
struct Layer {
    name: String,
    y: Arc<[f64]>,
    color: Option<Color>,
}

/// Areas stacked bottom to top in the order given, each layer with its own colour (the
/// palette's next by default) and legend entry. Hiding a layer in the legend restacks the rest.
///
/// ```
/// use rune_chart::{StackedArea, data::Series};
/// let a = Series::from_y([1.0, 2.0, 3.0]);
/// let b = Series::from_y([2.0, 2.0, 1.0]);
/// let stacked = StackedArea::new([("Weapons", a), ("Armour", b)]);
/// ```
#[derive(Clone)]
pub struct StackedArea {
    x: Arc<[f64]>,
    layers: Vec<Layer>,
    hidden: Vec<bool>,
    offset: StackOffset,
    curve: Curve,
    line: bool,
    opacity: f32,
}

impl StackedArea {
    /// Layers named and stacked in order, bottom first. They share the first series' x: every
    /// layer's values are taken by index (a missing or `NaN` value is a gap the layers above
    /// close over).
    pub fn new<N: Into<String>>(layers: impl IntoIterator<Item = (N, Series)>) -> StackedArea {
        let mut x = None;
        let layers: Vec<Layer> = layers
            .into_iter()
            .map(|(name, series)| {
                x.get_or_insert_with(|| series.x().clone());
                Layer { name: name.into(), y: series.y().clone(), color: None }
            })
            .collect();
        StackedArea {
            x: x.unwrap_or_else(|| Arc::from([])),
            hidden: vec![false; layers.len()],
            layers,
            offset: StackOffset::None,
            curve: Curve::Linear,
            line: true,
            opacity: 0.55,
        }
    }

    /// The layers' colours, bottom first, instead of the palette's.
    pub fn colors(mut self, colors: impl IntoIterator<Item = Color>) -> StackedArea {
        for (layer, color) in self.layers.iter_mut().zip(colors) {
            layer.color = Some(color);
        }
        self
    }

    /// How the stack is placed: from zero (default), or `StackOffset::Expand` for shares of
    /// 100% (as fractions 0 to 1).
    pub fn offset(mut self, offset: StackOffset) -> StackedArea {
        self.offset = offset;
        self
    }

    /// Straight (default), smooth (`Curve::MonotoneX`) or stepped (`Curve::Step(_)`) edges.
    pub fn curve(mut self, curve: Curve) -> StackedArea {
        self.curve = curve;
        self
    }

    /// Whether to stroke each layer's top edge (on by default).
    pub fn line(mut self, on: bool) -> StackedArea {
        self.line = on;
        self
    }

    /// The fills' opacity, 0 to 1 (0.55 by default).
    pub fn opacity(mut self, opacity: f32) -> StackedArea {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    fn layer_color(&self, k: usize) -> Color {
        self.layers[k].color.unwrap_or(Color::WHITE)
    }

    fn values(&self) -> Vec<&[f64]> {
        self.layers.iter().map(|l| l.y.as_ref()).collect()
    }

    /// One layer's band at the stacked `indices` as top and bottom polylines in points, split at
    /// gaps.
    fn runs(&self, frame: &Frame, indices: &[usize], bands: &[[f64; 2]]) -> Vec<Run> {
        let mut out = Vec::new();
        let (mut top, mut bottom) = (Vec::new(), Vec::new());
        for (&i, b) in indices.iter().zip(bands) {
            if b[0].is_finite() && b[1].is_finite() {
                top.push(frame.px(self.x[i], b[1]));
                bottom.push(frame.px(self.x[i], b[0]));
            } else if !top.is_empty() {
                out.push((std::mem::take(&mut top), std::mem::take(&mut bottom)));
            }
        }
        if !top.is_empty() {
            out.push((top, bottom));
        }
        out
    }
}

impl Plot for StackedArea {
    fn name(&self) -> &str {
        self.layers.first().map_or("", |l| &l.name)
    }

    fn color(&self) -> Color {
        self.layers.first().and_then(|l| l.color).unwrap_or(Color::WHITE)
    }

    fn set_default_color(&mut self, _: Color) {}

    fn prepare(&mut self, style: &ChartStyle, palette: usize, hidden: &BTreeSet<String>) {
        for (k, layer) in self.layers.iter_mut().enumerate() {
            layer.color.get_or_insert(style.color(palette + k));
            self.hidden[k] = hidden.contains(&layer.name);
        }
    }

    fn legend(&self) -> Vec<(String, Color)> {
        self.layers.iter().enumerate().map(|(k, l)| (l.name.clone(), self.layer_color(k))).collect()
    }

    fn x_extent(&self) -> Option<(f64, f64)> {
        Some((*self.x.first()?, *self.x.last()?))
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        let indices: Vec<usize> = visible_range(&self.x, x).collect();
        let bands = stack_at(&self.values(), &self.hidden, &indices, self.offset);
        let finite = bands.iter().flatten().flatten().copied().filter(|v| v.is_finite());
        finite.fold(None, |acc, v| Some(acc.map_or((v, v), |(lo, hi): (f64, f64)| (lo.min(v), hi.max(v)))))
    }

    fn draw(&self, frame: &Frame, mut shape: Shape) -> Shape {
        let range = visible_range(&self.x, frame.viewport.x);
        let shown = (0..self.layers.len()).filter(|&k| !self.hidden[k]);
        let picked = shown.map(|k| {
            let y = &self.layers[k].y[..self.layers[k].y.len().min(self.x.len())];
            decimate(
                &self.x[..y.len()],
                y,
                range.start.min(y.len())..range.end.min(y.len()),
                frame.viewport.x,
                frame.columns(),
            )
        });
        let indices = merge_indices(picked.collect::<Vec<_>>());
        let bands = stack_at(&self.values(), &self.hidden, &indices, self.offset);
        for (k, layer) in bands.iter().enumerate().filter(|(k, _)| !self.hidden[*k]) {
            let color = self.layer_color(k);
            for (top, bottom) in self.runs(frame, &indices, layer) {
                let (top, bottom) = (self.curve.polyline(&top, 0.25), self.curve.polyline(&bottom, 0.25));
                shape = shape.area(top.clone(), bottom, color.alpha(color.a * self.opacity));
                if self.line {
                    shape = shape.stroke(top, 1.5, color);
                }
            }
        }
        shape
    }

    fn snap(&self, x: f64) -> Option<f64> {
        nearest_x(&self.x, &self.x, x).map(|i| self.x[i])
    }

    fn hover(&self, frame: &Frame, x: f64, pointer: [f32; 2]) -> Option<HoverPoint> {
        self.hover_points(frame, x, pointer).into_iter().next()
    }

    fn hover_points(&self, frame: &Frame, x: f64, _: [f32; 2]) -> Vec<HoverPoint> {
        let Some(i) = nearest_x(&self.x, &self.x, x).filter(|&i| self.x[i] == x) else { return Vec::new() };
        let bands = stack_at(&self.values(), &self.hidden, &[i], self.offset);
        // Top layer first, as they sit on the chart.
        (0..self.layers.len())
            .rev()
            .filter_map(|k| {
                let (band, value) = (bands[k][0], self.layers[k].y.get(i).copied()?);
                (band[1].is_finite() && value.is_finite()).then(|| HoverPoint {
                    x,
                    dot: Some(frame.px(x, band[1])),
                    row: TooltipRow {
                        color: self.layer_color(k),
                        label: self.layers[k].name.clone(),
                        value: (frame.format_y)(value),
                    },
                })
            })
            .collect()
    }
}
