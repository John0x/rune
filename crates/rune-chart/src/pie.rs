//! [`PieChart`]: a pie or donut of named values, with outside labels on leader lines, a legend
//! that hides slices, and a tooltip on the slice under the pointer, which lifts out.

use std::rc::Rc;

use rune::{Color, El, Shape, col, el, row};

use crate::TooltipRow;
use crate::hit::slice_at;
use crate::layout::{LabelSide, PieOptions, PieSort, arc_polygon, leader_line, pie, polar};
use crate::overlay::{label, legend, line_height, tooltip, width};
use crate::style::ChartStyle;
use crate::widget::{Format, Hover, default_format};

/// How far a hovered slice moves out, in points.
const LIFT: f32 = 6.0;
/// A leader line's run out from the slice, then sideways, in points.
const ELBOW: f32 = 12.0;
const RUN: f32 = 16.0;
/// Space between a leader line's end and its label.
const LABEL_GAP: f32 = 4.0;

/// A pie or donut chart of named values.
///
/// ```no_run
/// use rune_chart::PieChart;
/// let _el = PieChart::new("spend").slice("Food", 42.0).slice("Rent", 120.0).donut(0.6).build();
/// ```
pub struct PieChart {
    key: String,
    slices: Vec<(String, f64, Option<Color>)>,
    donut: f32,
    pad: f64,
    sort: bool,
    labels: bool,
    legend: bool,
    format: Format,
    style: ChartStyle,
    height: f32,
}

impl PieChart {
    /// A pie keyed `key`: its hover and hidden slices live under it, so give each chart on a
    /// page its own.
    pub fn new(key: impl Into<String>) -> PieChart {
        PieChart {
            key: key.into(),
            slices: Vec::new(),
            donut: 0.0,
            pad: 0.0,
            sort: false,
            labels: false,
            legend: true,
            format: default_format(),
            style: ChartStyle::default(),
            height: 220.0,
        }
    }

    /// Adds a slice in the palette's next colour. Negative and non-finite values count as 0.
    pub fn slice(mut self, name: impl Into<String>, value: f64) -> PieChart {
        self.slices.push((name.into(), value, None));
        self
    }

    /// Adds a slice in `color`.
    pub fn slice_color(mut self, name: impl Into<String>, value: f64, color: Color) -> PieChart {
        self.slices.push((name.into(), value, Some(color)));
        self
    }

    /// Adds a slice per `(name, value)`, in palette colours.
    pub fn data<S: Into<String>>(mut self, slices: impl IntoIterator<Item = (S, f64)>) -> PieChart {
        self.slices.extend(slices.into_iter().map(|(name, value)| (name.into(), value, None)));
        self
    }

    /// A donut: the hole's radius as a fraction of the pie's (0 for a pie, the default).
    pub fn donut(mut self, inner_ratio: f32) -> PieChart {
        self.donut = if inner_ratio.is_finite() { inner_ratio.clamp(0.0, 0.95) } else { 0.0 };
        self
    }

    /// The angle left empty between slices, in radians (0 by default; 0.02 is a thin gap).
    pub fn pad_angle(mut self, radians: f64) -> PieChart {
        self.pad = if radians.is_finite() { radians.abs() } else { 0.0 };
        self
    }

    /// Whether slices go around largest first (off by default: in the order given).
    pub fn sort(mut self, sort: bool) -> PieChart {
        self.sort = sort;
        self
    }

    /// Whether to label slices outside the pie, on leader lines, with their name and share.
    pub fn labels(mut self, labels: bool) -> PieChart {
        self.labels = labels;
        self
    }

    /// Whether to show the legend (on by default); a click on an entry hides or shows its slice.
    pub fn legend(mut self, legend: bool) -> PieChart {
        self.legend = legend;
        self
    }

    /// How the tooltip writes a value (three significant digits by default).
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> PieChart {
        self.format = Rc::new(f);
        self
    }

    /// Colours and type.
    pub fn style(mut self, style: ChartStyle) -> PieChart {
        self.style = style;
        self
    }

    /// The plot's height in points (220 by default); the pie fits it, centred across the
    /// width, and the legend adds to it.
    pub fn height(mut self, height: f32) -> PieChart {
        self.height = if height.is_finite() { height.max(1.0) } else { 220.0 };
        self
    }

    /// The chart's elements.
    pub fn build(self) -> El {
        build(self)
    }
}

/// A slice as drawn: its input index, colour, value, angles and share of the total.
struct Drawn {
    index: usize,
    color: Color,
    value: f64,
    start: f64,
    end: f64,
    pad: f64,
    share: f64,
}

fn build(chart: PieChart) -> El {
    let style = &chart.style;
    let hover = rune::component_state::<Hover<usize>>(&format!("rune-chart-pie/{}", chart.key));
    let hidden = hover.hidden.borrow().clone();
    let color = |i: usize| chart.slices[i].2.unwrap_or_else(|| style.color(i));
    let shown: Vec<usize> = (0..chart.slices.len()).filter(|&i| !hidden.contains(&chart.slices[i].0)).collect();
    let values: Vec<f64> = shown.iter().map(|&i| chart.slices[i].1).collect();
    let sort = if chart.sort { PieSort::Descending } else { PieSort::None };
    let layout = pie(&values, &PieOptions { pad_angle: chart.pad, sort, ..PieOptions::default() });
    let total: f64 = layout.iter().map(|s| s.value).sum();
    let slices: Vec<Drawn> = layout
        .iter()
        .map(|s| Drawn {
            index: shown[s.index],
            color: color(shown[s.index]),
            value: s.value,
            start: s.start,
            end: s.end,
            pad: s.pad,
            share: if total > 0.0 { s.value / total } else { 0.0 },
        })
        .collect();

    let plot_key = format!("{}/plot", chart.key);
    let w = rune::memo::layout_size(&plot_key).map_or(480.0, |s| s[0]).max(1.0);
    let h = chart.height;
    let lh = line_height(style);
    let texts: Vec<(String, String)> =
        slices.iter().map(|s| (chart.slices[s.index].0.clone(), percent(s.share))).collect();
    let text_w = |(name, pct): &(String, String)| width(name, style.label) + 4.0 + width(pct, style.label);
    let room = if chart.labels {
        texts.iter().map(text_w).fold(0.0, f32::max).ceil() + ELBOW + RUN + LABEL_GAP + 4.0
    } else {
        LIFT + 2.0
    };
    let outer = (h / 2.0 - LIFT - if chart.labels { lh / 2.0 } else { 0.0 }).min(w / 2.0 - room).max(8.0);
    let inner = outer * chart.donut;
    let center = [w / 2.0, h / 2.0];
    let hovered = hover.get().filter(|(i, _)| slices.iter().any(|s| s.index == *i && s.value > 0.0));

    let mut shape = Shape::new();
    if total <= 0.0 {
        shape = shape.fill(arc_polygon(center, inner, outer, 0.0, std::f64::consts::TAU, 0.0, 0.25), style.grid);
    }
    for s in slices.iter().filter(|s| s.value > 0.0) {
        let lifted = hovered.is_some_and(|(i, _)| i == s.index);
        let c = if lifted { polar(center, (s.start + s.end) / 2.0, LIFT) } else { center };
        shape = shape.fill(arc_polygon(c, inner, outer, s.start, s.end, s.pad, 0.25), s.color);
    }
    let mut labels = Vec::new();
    if chart.labels && total > 0.0 {
        for (k, (points, side)) in label_lines(&slices, center, outer, h, lh).into_iter().enumerate() {
            let Some(points) = points else { continue };
            shape = shape.stroke(points.to_vec(), 1.0, style.muted.alpha(0.7));
            let (name, pct) = &texts[k];
            let end = points[2];
            let left = match side {
                LabelSide::Right => end[0] + LABEL_GAP,
                LabelSide::Left => end[0] - LABEL_GAP - text_w(&texts[k]),
            };
            labels.push(
                row()
                    .key(format!("label{}", slices[k].index))
                    .gap(4.0)
                    .absolute(left, end[1] - lh / 2.0)
                    .child(label(name, style, style.text).key("name"))
                    .child(label(pct, style, style.muted).key("share")),
            );
        }
    }

    let canvas = rune::canvas(shape).key("canvas").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    let mut area = el().key(plot_key).w_full().h(h).child(canvas).children(labels);
    if let Some((i, pointer)) = hovered
        && let Some(s) = slices.iter().find(|s| s.index == i)
    {
        let rows = [
            TooltipRow { color: s.color, label: "Value".into(), value: (chart.format)(s.value) },
            TooltipRow { color: s.color, label: "Share".into(), value: percent(s.share) },
        ];
        area = area.child(tooltip(style, [w, h], &chart.slices[i].0, &rows, pointer[0], pointer[1]));
    }
    let area = hover.attach(area, true, move |p| {
        slice_at(&layout, center, inner, outer + LIFT, p).filter(|&k| layout[k].value > 0.0).map(|k| shown[k])
    });

    let mut out = col().key(chart.key.clone()).w_full().gap(6.0);
    if chart.legend && !chart.slices.is_empty() {
        let items: Vec<(String, Color)> =
            chart.slices.iter().enumerate().map(|(i, s)| (s.0.clone(), color(i))).collect();
        out = out.child(legend(style, &items, &hover.hidden));
    }
    out.child(area)
}

/// A leader line (`None` for an empty slice) and the side its label goes on.
type Leader = (Option<[[f32; 2]; 3]>, LabelSide);

/// The leader line of each slice, its label end moved to a column beside the pie and pushed
/// apart from the labels on its side by at least a line height, within the plot's height.
fn label_lines(slices: &[Drawn], center: [f32; 2], outer: f32, h: f32, lh: f32) -> Vec<Leader> {
    let mut lines: Vec<Leader> = slices
        .iter()
        .map(|s| {
            let l = leader_line(center, outer, s.start, s.end, ELBOW, RUN);
            ((s.value > 0.0).then_some(l.points), l.side)
        })
        .collect();
    let end_y = |l: &Leader| l.0.map_or(0.0, |p| p[2][1]);
    for side in [LabelSide::Left, LabelSide::Right] {
        let mut on: Vec<usize> = (0..lines.len()).filter(|&k| lines[k].1 == side && lines[k].0.is_some()).collect();
        on.sort_by(|&a, &b| end_y(&lines[a]).total_cmp(&end_y(&lines[b])));
        let mut ys: Vec<f32> = on.iter().map(|&k| end_y(&lines[k])).collect();
        spread(&mut ys, lh, lh / 2.0, h - lh / 2.0);
        let sign = if side == LabelSide::Left { -1.0 } else { 1.0 };
        let column = center[0] + sign * (outer + ELBOW + RUN);
        for (&k, y) in on.iter().zip(ys) {
            if let Some(points) = &mut lines[k].0 {
                points[2] = [column, y];
            }
        }
    }
    lines
}

/// Pushes ascending label positions `ys` apart so neighbours are at least `gap` apart, moving
/// them as little as it can and keeping them within `top..=bottom` when they fit there (when
/// they don't, they run past `bottom`).
pub(crate) fn spread(ys: &mut [f32], gap: f32, top: f32, bottom: f32) {
    for i in 1..ys.len() {
        ys[i] = ys[i].max(ys[i - 1] + gap);
    }
    if let Some(last) = ys.last_mut() {
        *last = last.min(bottom);
    }
    for i in (0..ys.len().saturating_sub(1)).rev() {
        ys[i] = ys[i].min(ys[i + 1] - gap);
    }
    if let Some(first) = ys.first_mut() {
        *first = first.max(top);
    }
    for i in 1..ys.len() {
        ys[i] = ys[i].max(ys[i - 1] + gap);
    }
}

/// A share as a percentage: `32%`, or `0.4%` under 1 %.
pub(crate) fn percent(share: f64) -> String {
    let p = share * 100.0;
    if p > 0.0 && p < 1.0 { format!("{p:.1}%") } else { format!("{p:.0}%") }
}

#[cfg(test)]
#[path = "pie_tests.rs"]
mod tests;
