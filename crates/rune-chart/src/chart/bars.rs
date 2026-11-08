//! Vertical bars from zero: one series, several side by side, or several stacked.

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;

use rune::{Color, Shape};

use super::geometry::{bar_outline, bar_width, clamp_y, column_groups, min_spacing, stack_at};
use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::{Series, union};
use crate::decimate::visible_range;
use crate::hit::nearest_x;
use crate::layout::StackOffset;
use crate::style::ChartStyle;

#[derive(Clone)]
struct BarSeries {
    name: String,
    series: Series,
    color: Option<Color>,
}

/// Vertical bars centred on their x, from 0 up to their value (negative values go down): one
/// series ([`Bars::new`]), several side by side ([`Bars::grouped`]) or stacked
/// ([`Bars::stacked`]), each series with a legend entry. The hovered bar stays bright and the
/// rest dim. Bars thinner than a point merge into one per pixel column.
///
/// ```
/// use rune_chart::{Bars, data::Series};
/// let sold = Series::from_y([12.0, 30.0, 18.0]);
/// let bars = Bars::new("Sold", sold).width(0.6);
/// ```
#[derive(Clone)]
pub struct Bars {
    series: Vec<BarSeries>,
    stacked: bool,
    hidden: Vec<bool>,
    fraction: f32,
    radius: f32,
    color_by: Option<Rc<dyn Fn(f64, f64) -> Color>>,
    /// The smallest x spacing in data units, for the bars' width.
    spacing: Option<f64>,
}

/// How bars lay out in one view: the width of a group of bars in points, and of one bar.
struct Layout {
    group: f32,
    bar: f32,
    /// Whether bars are under a point wide and merge per pixel column.
    dense: bool,
    base: f32,
}

impl Bars {
    /// Bars for `series` (x ascending, `NaN` y no bar).
    pub fn new(name: impl Into<String>, series: Series) -> Bars {
        Bars::grouped([(name, series)])
    }

    /// Several series' bars side by side at each x, in the order given.
    pub fn grouped<N: Into<String>>(series: impl IntoIterator<Item = (N, Series)>) -> Bars {
        let series: Vec<BarSeries> =
            series.into_iter().map(|(name, series)| BarSeries { name: name.into(), series, color: None }).collect();
        let spacing = min_spacing(series.iter().map(|s| s.series.x().as_ref()));
        Bars {
            hidden: vec![false; series.len()],
            series,
            stacked: false,
            fraction: 0.7,
            radius: 2.0,
            color_by: None,
            spacing,
        }
    }

    /// Several series' bars stacked at each x, the first at the bottom; positive values stack up
    /// from zero and negative ones down. They share the first series' x: every series' values
    /// are taken by index.
    pub fn stacked<N: Into<String>>(series: impl IntoIterator<Item = (N, Series)>) -> Bars {
        let mut bars = Bars::grouped(series);
        bars.stacked = true;
        bars.radius = 0.0;
        bars.spacing = min_spacing(bars.series.first().map(|s| s.series.x().as_ref()));
        bars
    }

    /// The first series' colour, instead of the palette's.
    pub fn color(self, color: Color) -> Bars {
        self.colors([color])
    }

    /// The series' colours in order, instead of the palette's.
    pub fn colors(mut self, colors: impl IntoIterator<Item = Color>) -> Bars {
        for (s, color) in self.series.iter_mut().zip(colors) {
            s.color = Some(color);
        }
        self
    }

    /// Colours each bar from its x and value (volume green on a rising day, red on a falling
    /// one); the legend keeps the series' colours.
    pub fn color_by(mut self, f: impl Fn(f64, f64) -> Color + 'static) -> Bars {
        self.color_by = Some(Rc::new(f));
        self
    }

    /// The bars' width as a fraction of the smallest x spacing (0.7 by default); a group of
    /// bars shares it.
    pub fn width(mut self, fraction: f32) -> Bars {
        self.fraction = fraction;
        self
    }

    /// The radius of the corners at a bar's value end in points (2 by default, 0 for stacked
    /// bars), kept under a quarter of the bar's width.
    pub fn radius(mut self, radius: f32) -> Bars {
        self.radius = radius.max(0.0);
        self
    }

    fn shown(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.series.len()).filter(|&k| !self.hidden[k])
    }

    fn series_color(&self, k: usize) -> Color {
        self.series[k].color.unwrap_or(Color::WHITE)
    }

    /// Bar `(x, y)`'s colour in series `k`.
    fn bar_color(&self, k: usize, x: f64, y: f64) -> Color {
        self.color_by.as_ref().map_or_else(|| self.series_color(k), |f| f(x, y))
    }

    /// The x every stacked series shares: the first's.
    fn shared_x(&self) -> &[f64] {
        self.series.first().map_or(&[], |s| s.series.x().as_ref())
    }

    fn values(&self) -> Vec<&[f64]> {
        self.series.iter().map(|s| s.series.y().as_ref()).collect()
    }

    fn layout(&self, frame: &Frame) -> Layout {
        let slots = if self.stacked { 1 } else { self.shown().count().max(1) };
        let at = frame.viewport.x.0;
        let spacing = self.spacing.map(|s| (frame.x.map(at + s) - frame.x.map(at)).abs());
        let group = bar_width(spacing, self.fraction, frame.size[0] / 8.0);
        let dense = spacing.is_some_and(|s| s * self.fraction.clamp(0.0, 1.0) / (slots as f32) < 1.0);
        Layout { group, bar: (group / slots as f32).max(1.0), dense, base: clamp_y(frame.y.map(0.0), frame.size[1]) }
    }

    /// A bar from `from` to `to` (y in points) `width` wide at pixel x `center`.
    fn bar(&self, shape: Shape, center: f32, width: f32, [from, to]: [f32; 2], color: Color) -> Shape {
        if !(from.is_finite() && to.is_finite()) || from == to {
            return shape;
        }
        let (left, right) = (center - width / 2.0, center + width / 2.0);
        shape.fill(bar_outline(left, right, from, to, self.radius.min(width * 0.25)), color)
    }

    fn dim(frame: &Frame, color: Color, hovered: bool) -> Color {
        if frame.hover.is_some() && !hovered { color.alpha(color.a * 0.4) } else { color }
    }

    fn draw_grouped(&self, frame: &Frame, mut shape: Shape, l: &Layout) -> Shape {
        let shown: Vec<usize> = self.shown().collect();
        // Grouped bars keep a sliver between neighbours.
        let width = if shown.len() > 1 { (l.bar * 0.88).max(1.0) } else { l.bar };
        for (slot, &k) in shown.iter().enumerate() {
            let (x, y) = (self.series[k].series.x(), self.series[k].series.y());
            let offset = (slot as f32 + 0.5) * l.bar - l.group / 2.0;
            for i in visible_range(x, frame.viewport.x) {
                if !y[i].is_finite() {
                    continue;
                }
                let color = Bars::dim(frame, self.bar_color(k, x[i], y[i]), frame.hover == Some(x[i]));
                shape = self.bar(shape, frame.x.map(x[i]) + offset, width, [l.base, frame.y.map(y[i])], color);
            }
        }
        shape
    }

    fn draw_stacked(&self, frame: &Frame, mut shape: Shape, l: &Layout) -> Shape {
        let x = self.shared_x();
        let indices: Vec<usize> = visible_range(x, frame.viewport.x).collect();
        let bands = stack_at(&self.values(), &self.hidden, &indices, StackOffset::Diverging);
        for k in self.shown() {
            for (&i, b) in indices.iter().zip(&bands[k]) {
                let value = self.series[k].series.y().get(i).copied().unwrap_or(f64::NAN);
                let color = Bars::dim(frame, self.bar_color(k, x[i], value), frame.hover == Some(x[i]));
                shape = self.bar(shape, frame.x.map(x[i]), l.bar, [frame.y.map(b[0]), frame.y.map(b[1])], color);
            }
        }
        shape
    }

    /// Bars under a point wide, one per pixel column and series: from the lowest to the highest
    /// value in the column through zero, or a stacked series' lowest to highest bound.
    fn draw_dense(&self, frame: &Frame, mut shape: Shape) -> Shape {
        let column = |shape: Shape, g: &Range<usize>, x: &[f64], [lo, hi]: [f64; 2], color: Color| {
            let hovered = frame.hover.is_some_and(|h| x[g.clone()].contains(&h));
            let center = frame.x.map(x[g.start]).floor() + 0.5;
            self.bar(shape, center, 1.0, [frame.y.map(lo), frame.y.map(hi)], Bars::dim(frame, color, hovered))
        };
        if self.stacked {
            let x = self.shared_x();
            let range = visible_range(x, frame.viewport.x);
            let indices: Vec<usize> = range.clone().collect();
            let bands = stack_at(&self.values(), &self.hidden, &indices, StackOffset::Diverging);
            for g in column_groups(x, range.clone(), |v| frame.x.map(v)) {
                for k in self.shown() {
                    let local = g.start - range.start..g.end - range.start;
                    let Some(span) = extent(bands[k][local].iter().flatten().copied()) else { continue };
                    shape = column(shape, &g, x, [span.0, span.1], self.series_color(k));
                }
            }
            return shape;
        }
        for k in self.shown() {
            let (x, y) = (self.series[k].series.x(), self.series[k].series.y());
            for g in column_groups(x, visible_range(x, frame.viewport.x), |v| frame.x.map(v)) {
                let Some(span) = extent(y[g.clone()].iter().copied()) else { continue };
                // The column takes the colour of its tallest bar.
                let tallest =
                    g.clone().filter(|&i| y[i].is_finite()).max_by(|&a, &b| y[a].abs().total_cmp(&y[b].abs()));
                let color = tallest.map_or_else(|| self.series_color(k), |i| self.bar_color(k, x[i], y[i]));
                shape = column(shape, &g, x, [span.0.min(0.0), span.1.max(0.0)], color);
            }
        }
        shape
    }
}

/// The lowest and highest finite value, if any.
fn extent(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    values.filter(|v| v.is_finite()).fold(None, |acc, v| Some(acc.map_or((v, v), |(lo, hi)| (v.min(lo), v.max(hi)))))
}

impl Plot for Bars {
    fn name(&self) -> &str {
        self.series.first().map_or("", |s| &s.name)
    }

    fn color(&self) -> Color {
        self.series.first().and_then(|s| s.color).unwrap_or(Color::WHITE)
    }

    fn set_default_color(&mut self, _: Color) {}

    fn prepare(&mut self, style: &ChartStyle, palette: usize, hidden: &BTreeSet<String>) {
        for (k, s) in self.series.iter_mut().enumerate() {
            s.color.get_or_insert(style.color(palette + k));
            self.hidden[k] = hidden.contains(&s.name);
        }
    }

    fn legend(&self) -> Vec<(String, Color)> {
        self.series.iter().enumerate().map(|(k, s)| (s.name.clone(), self.series_color(k))).collect()
    }

    fn x_extent(&self) -> Option<(f64, f64)> {
        let extent = self.series.iter().fold(None, |acc, s| union(acc, s.series.x_extent()))?;
        let half = self.spacing.unwrap_or(0.0) / 2.0;
        Some((extent.0 - half, extent.1 + half))
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        let values = if self.stacked {
            let indices: Vec<usize> = visible_range(self.shared_x(), x).collect();
            let bands = stack_at(&self.values(), &self.hidden, &indices, StackOffset::Diverging);
            extent(bands.iter().flatten().flatten().copied())
        } else {
            self.shown().fold(None, |acc, k| {
                let s = &self.series[k].series;
                union(acc, s.y_extent_in(visible_range(s.x(), x)))
            })
        };
        values.map(|(lo, hi)| (lo.min(0.0), hi.max(0.0)))
    }

    fn draw(&self, frame: &Frame, shape: Shape) -> Shape {
        let layout = self.layout(frame);
        if layout.dense {
            self.draw_dense(frame, shape)
        } else if self.stacked {
            self.draw_stacked(frame, shape, &layout)
        } else {
            self.draw_grouped(frame, shape, &layout)
        }
    }

    fn snap(&self, x: f64) -> Option<f64> {
        self.shown()
            .filter_map(|k| {
                let s = &self.series[k].series;
                nearest_x(s.x(), s.y(), x).map(|i| s.x()[i])
            })
            .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
    }

    fn hover(&self, frame: &Frame, x: f64, pointer: [f32; 2]) -> Option<HoverPoint> {
        self.hover_points(frame, x, pointer).into_iter().next()
    }

    fn hover_points(&self, frame: &Frame, x: f64, _: [f32; 2]) -> Vec<HoverPoint> {
        let row = |k: usize, i: usize| {
            let s = &self.series[k].series;
            let (px, py) = s.point(i)?;
            (px == x && py.is_finite()).then(|| HoverPoint {
                x,
                dot: None,
                row: TooltipRow {
                    color: self.bar_color(k, px, py),
                    label: self.series[k].name.clone(),
                    value: (frame.format_y)(py),
                },
            })
        };
        if self.stacked {
            let Some(i) = nearest_x(self.shared_x(), self.shared_x(), x) else { return Vec::new() };
            // Top series first, as they sit on the chart.
            let shown: Vec<usize> = self.shown().collect();
            return shown.into_iter().rev().filter_map(|k| row(k, i)).collect();
        }
        self.shown()
            .filter_map(|k| {
                let s = &self.series[k].series;
                row(k, nearest_x(s.x(), s.y(), x)?)
            })
            .collect()
    }
}
