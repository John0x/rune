//! Candlesticks: a wick from low to high and a body from open to close per row of OHLC data.

use std::collections::BTreeSet;

use rune::{Color, LineCap, Shape};

use super::geometry::{bar_width, candle, column_groups, merge_ohlc, min_spacing};
use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::Ohlc;
use crate::decimate::visible_range;
use crate::hit::nearest_x;
use crate::style::ChartStyle;

/// A candlestick per row of [`Ohlc`] data: a wick from low to high and a body from open to
/// close, in the style's up colour when it closed at or above its open and its down colour
/// otherwise. Candles thinner than a point merge into one per pixel column (first open, highest
/// high, lowest low, last close). The tooltip shows the hovered candle's four values.
///
/// ```
/// use rune_chart::{Candles, data::Ohlc};
/// let rows = Ohlc::from_rows([(0.0, 10.0, 12.0, 9.0, 11.0), (1.0, 11.0, 11.5, 8.0, 9.0)]).unwrap();
/// let candles = Candles::new("Price", rows);
/// ```
#[derive(Clone)]
pub struct Candles {
    name: String,
    data: Ohlc,
    up: Option<Color>,
    down: Option<Color>,
    fraction: f32,
    spacing: Option<f64>,
}

impl Candles {
    /// Candles for `data` (x ascending; a row with a `NaN` value is skipped).
    pub fn new(name: impl Into<String>, data: Ohlc) -> Candles {
        let spacing = min_spacing([data.x().as_ref()]);
        Candles { name: name.into(), data, up: None, down: None, fraction: 0.7, spacing }
    }

    /// A rising and a falling candle's colours, instead of the style's `up` and `down`.
    pub fn colors(mut self, up: Color, down: Color) -> Candles {
        self.up = Some(up);
        self.down = Some(down);
        self
    }

    /// The bodies' width as a fraction of the smallest x spacing (0.7 by default).
    pub fn width(mut self, fraction: f32) -> Candles {
        self.fraction = fraction;
        self
    }

    pub fn data(&self) -> &Ohlc {
        &self.data
    }

    /// A candle's colour: up when it closed at or above its open.
    fn tone(&self, open: f64, close: f64) -> Color {
        let (up, down) = (self.up.unwrap_or(Color::WHITE), self.down.unwrap_or(Color::WHITE));
        if close >= open { up } else { down }
    }

    /// The row at `i`, when every value is finite.
    fn row(&self, i: usize) -> Option<[f64; 4]> {
        let (_, o, h, l, c) = self.data.row(i)?;
        [o, h, l, c].iter().all(|v| v.is_finite()).then_some([o, h, l, c])
    }

    /// One candle at pixel x `center`, `width` wide; under 3 points wide it is only a wick.
    fn candle(&self, frame: &Frame, shape: Shape, center: f32, width: f32, [o, h, l, c]: [f64; 4]) -> Shape {
        let color = self.tone(o, c);
        let ([top, bottom], [left, upper, right, lower]) = candle(center, width, [o, h, l, c].map(|v| frame.y.map(v)));
        let shape = shape.line(top, bottom, 1.0, LineCap::Butt, color);
        if width < 3.0 {
            return shape;
        }
        shape.fill(vec![[left, upper], [right, upper], [right, lower], [left, lower]], color)
    }
}

impl Plot for Candles {
    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> Color {
        self.up.unwrap_or(Color::WHITE)
    }

    fn set_default_color(&mut self, _: Color) {}

    fn prepare(&mut self, style: &ChartStyle, _: usize, _: &BTreeSet<String>) {
        self.up.get_or_insert(style.up);
        self.down.get_or_insert(style.down);
    }

    fn x_extent(&self) -> Option<(f64, f64)> {
        let (a, b) = self.data.x_extent()?;
        let half = self.spacing.unwrap_or(0.0) / 2.0;
        Some((a - half, b + half))
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        self.data.y_extent_in(visible_range(self.data.x(), x))
    }

    fn draw(&self, frame: &Frame, mut shape: Shape) -> Shape {
        let x = self.data.x();
        let range = visible_range(x, frame.viewport.x);
        let at = frame.viewport.x.0;
        let spacing = self.spacing.map(|s| (frame.x.map(at + s) - frame.x.map(at)).abs());
        if spacing.is_some_and(|s| s * self.fraction.clamp(0.0, 1.0) < 1.0) {
            let (o, h, l, c) = (self.data.open(), self.data.high(), self.data.low(), self.data.close());
            for g in column_groups(x, range, |v| frame.x.map(v)) {
                if let Some(merged) = merge_ohlc(o, h, l, c, g.clone()) {
                    shape = self.candle(frame, shape, frame.x.map(x[g.start]).floor() + 0.5, 1.0, merged);
                }
            }
            return shape;
        }
        let width = bar_width(spacing, self.fraction, frame.size[0] / 8.0);
        for i in range {
            if let Some(row) = self.row(i) {
                shape = self.candle(frame, shape, frame.x.map(x[i]), width, row);
            }
        }
        shape
    }

    fn snap(&self, x: f64) -> Option<f64> {
        nearest_x(self.data.x(), self.data.close(), x).map(|i| self.data.x()[i])
    }

    fn hover(&self, frame: &Frame, x: f64, pointer: [f32; 2]) -> Option<HoverPoint> {
        self.hover_points(frame, x, pointer).into_iter().next()
    }

    fn hover_points(&self, frame: &Frame, x: f64, _: [f32; 2]) -> Vec<HoverPoint> {
        let i = nearest_x(self.data.x(), self.data.close(), x).filter(|&i| self.data.x()[i] == x);
        let Some(row) = i.and_then(|i| self.row(i)) else { return Vec::new() };
        let color = self.tone(row[0], row[3]);
        ["Open", "High", "Low", "Close"]
            .into_iter()
            .zip(row)
            .map(|(label, v)| HoverPoint {
                x,
                dot: None,
                row: TooltipRow { color, label: label.to_owned(), value: (frame.format_y)(v) },
            })
            .collect()
    }
}
