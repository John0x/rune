//! What a chart draws: the [`Plot`] trait every series kind implements, and the [`Frame`] it
//! draws in.

use std::collections::BTreeSet;
use std::rc::Rc;

use rune::{Color, Shape};

use crate::scale::Continuous;
use crate::style::ChartStyle;
use crate::viewport::Viewport;

/// The plot area a [`Plot`] draws in: its size, the visible viewport and the scales from data to
/// the area's pixels (`[0, 0]` its top-left corner, y growing downward).
#[derive(Clone)]
pub struct Frame {
    /// The plot area's width and height in points.
    pub size: [f32; 2],
    pub viewport: Viewport,
    pub x: Rc<dyn Continuous>,
    pub y: Rc<dyn Continuous>,
    /// The x and y axes' value formats, for tooltips.
    pub format_x: Rc<dyn Fn(f64) -> String>,
    pub format_y: Rc<dyn Fn(f64) -> String>,
    /// The data x the crosshair has snapped to, while the pointer is over the chart (bars keep
    /// the hovered bar bright and dim the rest).
    pub hover: Option<f64>,
}

impl Frame {
    /// Data `(x, y)` in the plot area's points.
    pub fn px(&self, x: f64, y: f64) -> [f32; 2] {
        [self.x.map(x), self.y.map(y)]
    }

    /// Pixel columns across the plot area: how far a line decimates.
    pub fn columns(&self) -> usize {
        self.size[0].max(1.0).ceil() as usize
    }

    /// Whether data x lies in the visible x domain.
    pub fn x_visible(&self, x: f64) -> bool {
        let (a, b) = self.viewport.x;
        x >= a.min(b) && x <= a.max(b)
    }
}

/// A plot's answer to the pointer: the data point it shows there, where to put its dot (if any)
/// and its tooltip row.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverPoint {
    /// The point's data x (the crosshair snaps here).
    pub x: f64,
    /// Where to draw the hover dot, in the plot area's points; `None` for no dot (a bar).
    pub dot: Option<[f32; 2]>,
    /// The tooltip row: the series' colour, name and value.
    pub row: TooltipRow,
}

/// A text label a plot puts in the plot area (a rule's name), drawn over its shape.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotLabel {
    pub text: String,
    /// Where the label goes, in the plot area's points.
    pub at: [f32; 2],
    /// Which point of the label sits at `at`, as fractions of its width and height: `[0, 0]` its
    /// top-left corner, `[1, 1]` its bottom-right corner.
    pub anchor: [f32; 2],
    pub color: Color,
}

/// A row of a chart's tooltip.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipRow {
    pub color: Color,
    pub label: String,
    pub value: String,
}

/// Something a [`Chart`](crate::Chart) draws on its x and y axes: a line, an area, bars, candles,
/// points or a rule. Implement it for a series kind of your own.
pub trait Plot {
    /// The name the legend and the tooltip show.
    fn name(&self) -> &str;

    /// The colour of its swatch in the legend.
    fn color(&self) -> Color;

    /// Gives a plot without a colour of its own the style's colour for its place.
    fn set_default_color(&mut self, color: Color);

    /// Its data's x extent, or `None` if it has none (a horizontal rule).
    fn x_extent(&self) -> Option<(f64, f64)>;

    /// Its data's y extent over the visible x domain `x`, for fitting y to what's in view.
    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)>;

    /// Adds its drawing to `shape`, in `frame`'s points.
    fn draw(&self, frame: &Frame, shape: Shape) -> Shape;

    /// The data x nearest `x` that the crosshair snaps to, or `None` if this plot doesn't snap it.
    fn snap(&self, x: f64) -> Option<f64> {
        let _ = x;
        None
    }

    /// What it shows at data `x` (where the crosshair snapped) with the pointer at `pointer`.
    fn hover(&self, frame: &Frame, x: f64, pointer: [f32; 2]) -> Option<HoverPoint>;

    /// Whether its hover depends on where the pointer is, not only on the snapped x (scatter
    /// points), so the chart re-runs its view as the pointer moves.
    fn follows_pointer(&self) -> bool {
        false
    }

    /// Every point it shows at data `x`, each with its tooltip row: [`Plot::hover`]'s one by
    /// default. A plot of several series (stacked areas, grouped bars) or several values (a
    /// candle's open, high, low and close) gives one per row.
    fn hover_points(&self, frame: &Frame, x: f64, pointer: [f32; 2]) -> Vec<HoverPoint> {
        self.hover(frame, x, pointer).into_iter().collect()
    }

    /// Whether it gets a legend entry.
    fn in_legend(&self) -> bool {
        true
    }

    /// Its legend entries, `(name, colour)` each: its name and colour (none when not
    /// [`in_legend`](Plot::in_legend)) by default, or one per series for a plot of several. The
    /// legend hides an entry by name; a plot whose entries are all hidden isn't drawn.
    fn legend(&self) -> Vec<(String, Color)> {
        if self.in_legend() { vec![(self.name().to_owned(), self.color())] } else { Vec::new() }
    }

    /// Prepares the plot before the chart lays it out, after
    /// [`set_default_color`](Plot::set_default_color): the chart's `style` (candles take its up
    /// and down colours), the palette index of its first series (`style.color(palette)`; the
    /// count of legend entries before it), and the names the legend has hidden, which a plot of
    /// several series leaves out itself. Does nothing by default.
    fn prepare(&mut self, style: &ChartStyle, palette: usize, hidden: &BTreeSet<String>) {
        let _ = (style, palette, hidden);
    }

    /// Text labels to draw in the plot area (a rule's name); none by default.
    fn labels(&self, frame: &Frame) -> Vec<PlotLabel> {
        let _ = frame;
        Vec::new()
    }
}

/// Whether the legend has hidden all of `plot`'s entries, so the chart skips it.
pub(crate) fn hidden_whole(plot: &dyn Plot, hidden: &BTreeSet<String>) -> bool {
    if hidden.is_empty() {
        return false;
    }
    let legend = plot.legend();
    !legend.is_empty() && legend.iter().all(|(name, _)| hidden.contains(name))
}
