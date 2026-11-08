//! [`HeatmapChart`]: a grid of values as coloured cells, with row and column labels, a colour
//! legend, and an outline and tooltip on the cell under the pointer.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use rune::{Color, El, Gradient, LineCap, Shape, col, el, row};

use crate::TooltipRow;
use crate::data::Grid;
use crate::hit::cell_at;
use crate::overlay::{label, line_height, width};
use crate::scale::Band;
use crate::style::ChartStyle;
use crate::widget::{Format, Hover, default_format, mix_oklab, tooltip_beside};

/// Space between labels and the grid, and between column labels.
const GAP: f32 = 6.0;
/// The colour legend's bar.
const BAR: [f32; 2] = [160.0, 8.0];

/// A heat map: a value per column and row, coloured from low to high.
///
/// ```no_run
/// use rune_chart::HeatmapChart;
/// use rune_chart::data::Grid;
/// let grid = Grid::from_fn(24, 7, |hour, day| Some((hour * day) as f64));
/// let _el = HeatmapChart::new("week")
///     .columns((0..24).map(|h| format!("{h:02}")))
///     .rows(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"])
///     .grid(grid)
///     .build();
/// ```
pub struct HeatmapChart {
    key: String,
    columns: Vec<String>,
    rows: Vec<String>,
    grid: Grid,
    stops: Vec<Color>,
    domain: Option<(f64, f64)>,
    gap: f32,
    radius: f32,
    legend: bool,
    format: Format,
    style: ChartStyle,
    height: f32,
}

impl HeatmapChart {
    /// A heat map keyed `key`: its hover lives under it, so give each chart on a page its own.
    pub fn new(key: impl Into<String>) -> HeatmapChart {
        HeatmapChart {
            key: key.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            grid: Grid::default(),
            stops: Vec::new(),
            domain: None,
            gap: 2.0,
            radius: 2.0,
            legend: true,
            format: default_format(),
            style: ChartStyle::default(),
            height: 200.0,
        }
    }

    /// The columns' labels, left to right.
    pub fn columns<S: Into<String>>(mut self, labels: impl IntoIterator<Item = S>) -> HeatmapChart {
        self.columns = labels.into_iter().map(Into::into).collect();
        self
    }

    /// The rows' labels, top to bottom.
    pub fn rows<S: Into<String>>(mut self, labels: impl IntoIterator<Item = S>) -> HeatmapChart {
        self.rows = labels.into_iter().map(Into::into).collect();
        self
    }

    /// The values: cell `(column, row)`; an empty cell is drawn faint. The grid's size sets the
    /// chart's columns and rows; labels it lacks are left blank.
    pub fn grid(mut self, grid: Grid) -> HeatmapChart {
        self.grid = grid;
        self
    }

    /// A sequential scale from `low` (the domain's minimum) to `high` (its maximum), mixed in
    /// Oklab. By default the style's first palette colour, from faint to full.
    pub fn colors(mut self, low: Color, high: Color) -> HeatmapChart {
        self.stops = vec![low, high];
        self
    }

    /// A diverging scale: `low` at the domain's minimum, `mid` at its middle, `high` at its
    /// maximum. Give a [`domain`](Self::domain) centred on the value that should read as `mid`
    /// (`-x..x` for changes around zero).
    pub fn diverging(mut self, low: Color, mid: Color, high: Color) -> HeatmapChart {
        self.stops = vec![low, mid, high];
        self
    }

    /// The values the colour scale spans (by default the grid's smallest to largest); values
    /// outside it take the end colours.
    pub fn domain(mut self, min: f64, max: f64) -> HeatmapChart {
        self.domain = (min.is_finite() && max.is_finite()).then_some((min, max));
        self
    }

    /// The space between cells in points (2 by default).
    pub fn cell_gap(mut self, gap: f32) -> HeatmapChart {
        self.gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
        self
    }

    /// The cells' corner radius in points (2 by default).
    pub fn cell_radius(mut self, radius: f32) -> HeatmapChart {
        self.radius = if radius.is_finite() { radius.max(0.0) } else { 0.0 };
        self
    }

    /// Whether to show the colour legend under the grid (on by default).
    pub fn legend(mut self, legend: bool) -> HeatmapChart {
        self.legend = legend;
        self
    }

    /// How the tooltip and the legend write a value (three significant digits by default).
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> HeatmapChart {
        self.format = Rc::new(f);
        self
    }

    /// Colours and type.
    pub fn style(mut self, style: ChartStyle) -> HeatmapChart {
        self.style = style;
        self
    }

    /// The grid's height in points (200 by default); labels and the legend add to it.
    pub fn height(mut self, height: f32) -> HeatmapChart {
        self.height = if height.is_finite() { height.max(1.0) } else { 200.0 };
        self
    }

    /// The chart's elements.
    pub fn build(self) -> El {
        build(self)
    }
}

/// Colours spaced evenly over `[0, 1]`, mixed in Oklab between neighbours.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ramp(pub(crate) Vec<Color>);

impl Ramp {
    /// The colour at `t` (clamped to `[0, 1]`); transparent without stops.
    pub(crate) fn at(&self, t: f32) -> Color {
        let stops = &self.0;
        match stops.len() {
            0 => Color::TRANSPARENT,
            1 => stops[0],
            n => {
                let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } * (n - 1) as f32;
                let i = (t.floor() as usize).min(n - 2);
                mix_oklab(stops[i], stops[i + 1], t - i as f32)
            }
        }
    }
}

/// Where `v` lies in `(lo, hi)`, from 0 to 1, clamped; 0.5 when the domain is a single value.
pub(crate) fn fraction(v: f64, (lo, hi): (f64, f64)) -> f32 {
    if hi == lo { 0.5 } else { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 }
}

/// Every how many columns a label fits: labels at most `widest` points wide, `gap` apart, on
/// columns `step` points apart.
pub(crate) fn label_stride(step: f32, widest: f32, gap: f32) -> usize {
    if !(step > 0.0 && step.is_finite()) {
        return 1;
    }
    (((widest + gap) / step).ceil() as usize).max(1)
}

/// A rectangle with corners rounded by `r` (at most half its shorter side), clockwise from its
/// top-left, as a polygon to fill.
pub(crate) fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r < 0.25 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    let corners = [[x + w - r, y + r], [x + w - r, y + h - r], [x + r, y + h - r], [x + r, y + r]];
    let segments = 4;
    let mut out = Vec::with_capacity(4 * (segments + 1));
    for (k, c) in corners.iter().enumerate() {
        // Each corner from its start angle, a quarter turn on: -90° (up) for the top-right.
        let a0 = -FRAC_PI_2 + k as f32 * FRAC_PI_2;
        for s in 0..=segments {
            let a = a0 + FRAC_PI_2 * s as f32 / segments as f32;
            out.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    out
}

fn build(chart: HeatmapChart) -> El {
    let style = &chart.style;
    let hover = rune::component_state::<Hover<(usize, usize)>>(&format!("rune-chart-heatmap/{}", chart.key));
    let (nc, nr) = (chart.grid.columns(), chart.grid.rows());
    let domain = chart.domain.or_else(|| chart.grid.extent()).unwrap_or((0.0, 1.0));
    let ramp = Ramp(if chart.stops.is_empty() {
        let c = style.color(0);
        vec![mix_oklab(style.grid, c, 0.15), c]
    } else {
        chart.stops.clone()
    });

    let lh = line_height(style);
    let row_label = |r: usize| chart.rows.get(r).map_or("", String::as_str);
    let column_label = |c: usize| chart.columns.get(c).map_or("", String::as_str);
    let gutter = (0..nr).map(|r| width(row_label(r), style.label)).fold(0.0, f32::max).ceil();
    let gutter = if gutter > 0.0 { gutter + GAP } else { 0.0 };
    let plot_key = format!("{}/plot", chart.key);
    let w = rune::memo::layout_size(&plot_key).map_or(480.0, |s| s[0]).max(1.0);
    let h = chart.height;
    let grid_w = (w - gutter).max(1.0);
    let columns = Band::new(nc, (gutter, w));
    let rows = Band::new(nr, (0.0, h));
    let hovered = hover.get().filter(|((c, r), _)| *c < nc && *r < nr);

    let mut shape = Shape::new();
    let half = chart.gap / 2.0;
    let cell_rect = |c: usize, r: usize| {
        let (x, y) = (columns.position(c).unwrap_or(0.0), rows.position(r).unwrap_or(0.0));
        let (cw, ch) = ((columns.step() - chart.gap).max(1.0), (rows.step() - chart.gap).max(1.0));
        (x + half, y + half, cw, ch)
    };
    for r in 0..nr {
        for c in 0..nc {
            let (x, y, cw, ch) = cell_rect(c, r);
            // An empty cell is only outlined, so it never reads as a low value.
            shape = match chart.grid.get(c, r) {
                Some(v) => shape.fill(rounded_rect(x, y, cw, ch, chart.radius), ramp.at(fraction(v, domain))),
                None => {
                    let outline = rounded_rect(x + 0.5, y + 0.5, cw - 1.0, ch - 1.0, chart.radius);
                    shape.stroke_with(outline, true, 1.0, LineCap::Round, style.grid)
                }
            };
        }
    }
    if let Some(((c, r), _)) = hovered {
        let (x, y, cw, ch) = cell_rect(c, r);
        let outline = rounded_rect(x - 1.0, y - 1.0, cw + 2.0, ch + 2.0, chart.radius + 1.0);
        shape = shape.stroke_with(outline, true, 1.5, LineCap::Round, style.text);
    }

    let row_labels = (0..nr).map(|r| {
        let s = row_label(r);
        let top = rows.center(r).unwrap_or(0.0) - lh / 2.0;
        let muted = hovered.is_none_or(|((_, hr), _)| hr != r);
        label(s, style, if muted { style.muted } else { style.text })
            .key(format!("row{r}"))
            .absolute(gutter - GAP - width(s, style.label), top)
    });
    let canvas = rune::canvas(shape).key("canvas").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    let mut area = el().key(plot_key).w_full().h(h).child(canvas).children(row_labels);
    if let Some(((c, r), _)) = hovered {
        let (x, y, cw, ch) = cell_rect(c, r);
        let (value, swatch) = match chart.grid.get(c, r) {
            Some(v) => ((chart.format)(v), ramp.at(fraction(v, domain))),
            None => ("–".to_owned(), style.grid),
        };
        let title = match (row_label(r), column_label(c)) {
            ("", col) => col.to_owned(),
            (row, "") => row.to_owned(),
            (row, col) => format!("{row} · {col}"),
        };
        let rows = [TooltipRow { color: swatch, label: "Value".into(), value }];
        area = area.child(tooltip_beside(style, [w, h], &title, &rows, (x - 2.0, x + cw + 2.0), y + ch / 2.0));
    }
    let area = hover.attach(area, false, move |p| {
        let (c, r) = cell_at(&columns, &rows, p)?;
        let (x, y) = (columns.position(c)?, rows.position(r)?);
        // Only the cell itself, not the gap around it, so the outline matches what's hovered.
        let inside = p[0] >= x && p[0] <= x + columns.step() && p[1] >= y && p[1] <= y + rows.step();
        inside.then_some((c, r))
    });

    // Column labels, every `stride`th, centred under their columns and kept inside the grid.
    let widest = (0..nc).map(|c| width(column_label(c), style.label)).fold(0.0, f32::max);
    let stride = label_stride(columns.step(), widest, GAP);
    let column_labels = (0..nc).step_by(stride).map(|c| {
        let s = column_label(c);
        let lw = width(s, style.label);
        let left = (columns.center(c).unwrap_or(0.0) - lw / 2.0).clamp(gutter, (gutter + grid_w - lw).max(gutter));
        let muted = hovered.is_none_or(|((hc, _), _)| hc != c);
        label(s, style, if muted { style.muted } else { style.text }).key(format!("col{c}")).absolute(left, 0.0)
    });
    let mut out = col()
        .key(chart.key.clone())
        .w_full()
        .gap(6.0)
        .child(area)
        .child(el().key("columns").w_full().h(lh).children(column_labels));
    if chart.legend && chart.grid.extent().is_some() {
        out = out.child(color_legend(style, &ramp, domain, &chart.format, gutter));
    }
    out
}

/// A bar of the colour scale between the domain's ends, labelled with them.
fn color_legend(style: &ChartStyle, ramp: &Ramp, (lo, hi): (f64, f64), format: &Format, indent: f32) -> El {
    // One gradient per pair of neighbouring stops (one for a sequential ramp, two for a
    // diverging one), side by side: pieces that overlapped would blend translucent colours twice.
    let pieces = ramp.0.len().saturating_sub(1).max(1);
    let mut shape = Shape::new();
    for k in 0..pieces {
        let (a, b) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
        let (x0, x1) = (a * BAR[0], b * BAR[0]);
        let gradient = Gradient::horizontal(x0, ramp.at(a), x1, ramp.at(b));
        shape = shape.fill_gradient(vec![[x0, 0.0], [x1, 0.0], [x1, BAR[1]], [x0, BAR[1]]], gradient);
    }
    let bar = rune::canvas(shape).key("bar").size(BAR[0], BAR[1]).clip_rounded(BAR[1] / 2.0);
    row()
        .key("color-legend")
        .pl(indent)
        .gap(8.0)
        .items_center()
        .child(label(&format(lo), style, style.muted).key("min"))
        .child(bar)
        .child(label(&format(hi), style, style.muted).key("max"))
}

#[cfg(test)]
#[path = "heatmap_tests.rs"]
mod tests;
