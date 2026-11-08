//! [`RadarChart`]: series of values on axes around a circle, over grid rings, with a legend
//! that hides series and a tooltip of every series' value on the axis nearest the pointer.

use std::f64::consts::TAU;
use std::rc::Rc;

use rune::{Color, El, LineCap, Shape, col, el};

use crate::TooltipRow;
use crate::hit::radar_axis_at;
use crate::layout::{polar, radar_points, radar_ring};
use crate::overlay::{label, legend, line_height, width};
use crate::scale::Linear;
use crate::style::ChartStyle;
use crate::widget::{Format, Hover, default_format, tooltip_beside};

/// Space between the outer ring and an axis label, in points.
const LABEL_GAP: f32 = 8.0;

/// A radar (spider) chart: one axis per variable, one polygon per series.
///
/// ```no_run
/// use rune_chart::RadarChart;
/// let _el = RadarChart::new("stats")
///     .axes(["Speed", "Power", "Range", "Armor", "Cost"])
///     .series("Scout", vec![9.0, 3.0, 6.0, 2.0, 4.0])
///     .series("Tank", vec![2.0, 8.0, 4.0, 9.0, 7.0])
///     .build();
/// ```
pub struct RadarChart {
    key: String,
    axes: Vec<String>,
    series: Vec<(String, Vec<f64>, Option<Color>)>,
    max: Option<f64>,
    levels: usize,
    legend: bool,
    format: Format,
    style: ChartStyle,
    height: f32,
}

impl RadarChart {
    /// A radar chart keyed `key`: its hover and hidden series live under it, so give each chart
    /// on a page its own.
    pub fn new(key: impl Into<String>) -> RadarChart {
        RadarChart {
            key: key.into(),
            axes: Vec::new(),
            series: Vec::new(),
            max: None,
            levels: 4,
            legend: true,
            format: default_format(),
            style: ChartStyle::default(),
            height: 260.0,
        }
    }

    /// The axes' names, clockwise from twelve o'clock.
    pub fn axes<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> RadarChart {
        self.axes = names.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a series in the palette's next colour: value `i` on axis `i` (missing and
    /// non-finite values sit at the center).
    pub fn series(mut self, name: impl Into<String>, values: Vec<f64>) -> RadarChart {
        self.series.push((name.into(), values, None));
        self
    }

    /// Adds a series in `color`.
    pub fn series_color(mut self, name: impl Into<String>, values: Vec<f64>, color: Color) -> RadarChart {
        self.series.push((name.into(), values, Some(color)));
        self
    }

    /// The value at the outer ring (by default a nice value at or above the largest shown).
    pub fn max(mut self, max: f64) -> RadarChart {
        self.max = (max.is_finite() && max > 0.0).then_some(max);
        self
    }

    /// The number of grid rings (4 by default, at least 1).
    pub fn levels(mut self, levels: usize) -> RadarChart {
        self.levels = levels.clamp(1, 20);
        self
    }

    /// Whether to show the legend (on by default); a click on an entry hides or shows its
    /// series.
    pub fn legend(mut self, legend: bool) -> RadarChart {
        self.legend = legend;
        self
    }

    /// How ring labels and the tooltip write a value (three significant digits by default).
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> RadarChart {
        self.format = Rc::new(f);
        self
    }

    /// Colours and type.
    pub fn style(mut self, style: ChartStyle) -> RadarChart {
        self.style = style;
        self
    }

    /// The plot's height in points (260 by default), axis labels included; the legend adds to
    /// it.
    pub fn height(mut self, height: f32) -> RadarChart {
        self.height = if height.is_finite() { height.max(1.0) } else { 260.0 };
        self
    }

    /// The chart's elements.
    pub fn build(self) -> El {
        build(self)
    }
}

fn build(chart: RadarChart) -> El {
    let style = &chart.style;
    let hover = rune::component_state::<Hover<usize>>(&format!("rune-chart-radar/{}", chart.key));
    let hidden = hover.hidden.borrow().clone();
    let n = chart.axes.len();
    let color = |i: usize| chart.series[i].2.unwrap_or_else(|| style.color(i));
    // Each shown series with one value per axis.
    let shown: Vec<(usize, Vec<f64>)> = (0..chart.series.len())
        .filter(|&i| !hidden.contains(&chart.series[i].0))
        .map(|i| (i, (0..n).map(|a| chart.series[i].1.get(a).copied().unwrap_or(f64::NAN)).collect()))
        .collect();
    let largest = shown.iter().flat_map(|(_, v)| v.iter().copied()).filter(|v| v.is_finite()).fold(0.0, f64::max);
    let max = chart.max.unwrap_or_else(|| nice_max(largest, chart.levels));

    let plot_key = format!("{}/plot", chart.key);
    let w = rune::memo::layout_size(&plot_key).map_or(480.0, |s| s[0]).max(1.0);
    let h = chart.height;
    let lh = line_height(style);
    let label_w = chart.axes.iter().map(|a| width(a, style.label)).fold(0.0, f32::max).ceil();
    let outer = (h / 2.0 - lh - LABEL_GAP).min(w / 2.0 - label_w - LABEL_GAP - 4.0).max(10.0);
    let center = [w / 2.0, h / 2.0];
    let radius = Linear::new((0.0, max), (0.0, outer));
    let angles: Vec<f64> = (0..n).map(|i| TAU * i as f64 / n.max(1) as f64).collect();
    let hovered = hover.get().filter(|(a, _)| *a < n);

    let mut shape = Shape::new();
    for k in 1..=chart.levels {
        let r = outer * k as f32 / chart.levels as f32;
        shape = shape.stroke_with(radar_ring(center, n, r, 0.0), true, 1.0, LineCap::Butt, style.grid);
    }
    for (a, &angle) in angles.iter().enumerate() {
        let on = hovered.is_some_and(|(h, _)| h == a);
        let (lw, c) = if on { (1.5, style.crosshair) } else { (1.0, style.grid) };
        shape = shape.line(center, polar(center, angle, outer), lw, LineCap::Butt, c);
    }
    for (i, values) in &shown {
        let points = radar_points(center, values, &radius, 0.0);
        shape =
            shape.fill(points.clone(), color(*i).alpha(0.18)).stroke_with(points, true, 2.0, LineCap::Round, color(*i));
    }
    if let Some((a, _)) = hovered {
        for (i, values) in &shown {
            let r = if values[a].is_finite() { radius.map(values[a]).max(0.0) } else { 0.0 };
            let p = polar(center, angles[a], r);
            shape = shape.dot(p, 4.5, style.tooltip).dot(p, 3.0, color(*i));
        }
    }

    let mut labels = Vec::new();
    for (i, (name, &angle)) in chart.axes.iter().zip(&angles).enumerate() {
        let on = hovered.is_some_and(|(h, _)| h == i);
        let [x, y] = polar(center, angle, outer + LABEL_GAP);
        let [left, top] = axis_label_origin([x, y], angle, width(name, style.label), lh);
        labels.push(
            label(name, style, if on { style.text } else { style.muted }).key(format!("axis{i}")).absolute(left, top),
        );
    }
    if n > 0 {
        for k in 1..=chart.levels {
            let v = max * k as f64 / chart.levels as f64;
            let r = outer * k as f32 / chart.levels as f32;
            let s = (chart.format)(v);
            let el = label(&s, style, style.muted.alpha(0.8)).key(format!("ring{k}"));
            labels.push(el.absolute(center[0] + 4.0, center[1] - r));
        }
    }

    let canvas = rune::canvas(shape).key("canvas").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    let mut area = el().key(plot_key).w_full().h(h).child(canvas).children(labels);
    if let Some((a, _)) = hovered
        && !shown.is_empty()
    {
        let rows: Vec<TooltipRow> = shown
            .iter()
            .map(|(i, values)| TooltipRow {
                color: color(*i),
                label: chart.series[*i].0.clone(),
                value: if values[a].is_finite() { (chart.format)(values[a]) } else { "–".into() },
            })
            .collect();
        // Beside the radar and its labels, level with the axis' tip, so it covers neither the
        // axis nor its dots.
        let reach = outer + LABEL_GAP + label_w + 4.0;
        let y = polar(center, angles[a], outer)[1];
        area =
            area.child(tooltip_beside(style, [w, h], &chart.axes[a], &rows, (center[0] - reach, center[0] + reach), y));
    }
    let reach = outer + LABEL_GAP + lh;
    let area = hover.attach(area, false, move |p| {
        let near = (p[0] - center[0]).hypot(p[1] - center[1]) <= reach;
        near.then(|| radar_axis_at(n, 0.0, center, p)).flatten()
    });

    let mut out = col().key(chart.key.clone()).w_full().gap(6.0);
    if chart.legend && !chart.series.is_empty() {
        let items: Vec<(String, Color)> =
            chart.series.iter().enumerate().map(|(i, s)| (s.0.clone(), color(i))).collect();
        out = out.child(legend(style, &items, &hover.hidden));
    }
    out.child(area)
}

/// The top-left corner of an axis label `w` wide and `lh` tall whose anchor is `p`, just
/// outside the outer ring at `angle`: labels on the right start at it, labels on the left end at
/// it, labels at the top sit above it and at the bottom below it.
pub(crate) fn axis_label_origin(p: [f32; 2], angle: f64, w: f32, lh: f32) -> [f32; 2] {
    let (sin, cos) = (angle.sin() as f32, angle.cos() as f32);
    let left = if sin > 0.15 {
        p[0]
    } else if sin < -0.15 {
        p[0] - w
    } else {
        p[0] - w / 2.0
    };
    let top = if cos > 0.15 {
        p[1] - lh
    } else if cos < -0.15 {
        p[1]
    } else {
        p[1] - lh / 2.0
    };
    [left, top]
}

/// A nice value for the outer ring: `levels` rings of a round step (1, 1.5, 2, 2.5, 3, 4, 5,
/// 6 or 8 × 10ᵏ) reaching at least `largest`; `levels` when `largest` isn't positive.
pub(crate) fn nice_max(largest: f64, levels: usize) -> f64 {
    const STEPS: [f64; 10] = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];
    let levels = levels.max(1) as f64;
    if !(largest.is_finite() && largest > 0.0) {
        return levels;
    }
    let raw = largest / levels;
    let power = 10f64.powf(raw.log10().floor());
    let step = STEPS.iter().map(|m| m * power).find(|s| *s >= raw * (1.0 - 1e-9));
    step.unwrap_or(10.0 * power) * levels
}

#[cfg(test)]
#[path = "radar_tests.rs"]
mod tests;
