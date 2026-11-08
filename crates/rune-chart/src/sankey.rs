//! [`SankeyChart`]: flows between nodes as bands as wide as their value, with node labels, a
//! tooltip on the link or node under the pointer (its flows lit, the rest faded), and pan and
//! zoom.
//!
//! Drag pans; pinch, or ⌘/Ctrl + wheel, zooms at the pointer (a plain wheel scrolls the page);
//! a double click resets. The chart keeps its hover and zoom itself, under its key.

use std::cell::Cell;
use std::rc::Rc;

use rune::{Color, Cursor, Drag, El, Gradient, Phase, Shape, Tx, WheelEvent, col, el, row};

use crate::TooltipRow;
use crate::layout::{SankeyAlign, SankeyLink, SankeyOptions, sankey};
use crate::overlay::{label, line_height, tooltip, width};
use crate::style::ChartStyle;
use crate::widget::{Format, Hover, default_format};

/// Space between a node and its label, in points.
const LABEL_GAP: f32 = 6.0;
/// Zoom limits.
const MIN_SCALE: f32 = 0.5;
const MAX_SCALE: f32 = 8.0;
/// How much of the diagram stays in view however far it is panned, in points.
const KEEP: f32 = 48.0;

/// A sankey diagram: nodes in columns by their place in the flow, joined by links.
///
/// ```no_run
/// use rune_chart::SankeyChart;
/// let _el = SankeyChart::new("budget")
///     .node("Salary", None)
///     .node("Budget", None)
///     .node("Rent", None)
///     .node("Food", None)
///     .link(0, 1, 3000.0)
///     .link(1, 2, 1200.0)
///     .link(1, 3, 600.0)
///     .build();
/// ```
pub struct SankeyChart {
    key: String,
    nodes: Vec<(String, Option<Color>)>,
    links: Vec<SankeyLink>,
    align: SankeyAlign,
    node_width: f32,
    node_padding: f32,
    format: Format,
    style: ChartStyle,
    height: f32,
}

impl SankeyChart {
    /// A sankey keyed `key`: its hover and zoom live under it, so give each chart on a page its
    /// own.
    pub fn new(key: impl Into<String>) -> SankeyChart {
        SankeyChart {
            key: key.into(),
            nodes: Vec::new(),
            links: Vec::new(),
            align: SankeyAlign::Justify,
            node_width: 12.0,
            node_padding: 12.0,
            format: default_format(),
            style: ChartStyle::default(),
            height: 320.0,
        }
    }

    /// Adds a node, in `color` or (with `None`) the palette's colour for its index. Links name
    /// nodes by their index, in the order added.
    pub fn node(mut self, name: impl Into<String>, color: impl Into<Option<Color>>) -> SankeyChart {
        self.nodes.push((name.into(), color.into()));
        self
    }

    /// Adds a flow of `value` from node `source` to node `target`. Links to a node that doesn't
    /// exist, from a node to itself, or of a value that isn't positive are left out.
    pub fn link(mut self, source: usize, target: usize, value: f64) -> SankeyChart {
        self.links.push(SankeyLink::new(source, target, value));
        self
    }

    /// Which column each node goes in ([`SankeyAlign::Justify`] by default: sinks at the right).
    pub fn align(mut self, align: SankeyAlign) -> SankeyChart {
        self.align = align;
        self
    }

    /// The nodes' width in points (12 by default).
    pub fn node_width(mut self, width: f32) -> SankeyChart {
        self.node_width = if width.is_finite() { width.max(1.0) } else { 12.0 };
        self
    }

    /// The space between nodes in a column, in points (12 by default).
    pub fn node_padding(mut self, padding: f32) -> SankeyChart {
        self.node_padding = if padding.is_finite() { padding.max(0.0) } else { 12.0 };
        self
    }

    /// How labels and the tooltip write a value (three significant digits by default).
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> SankeyChart {
        self.format = Rc::new(f);
        self
    }

    /// Colours and type.
    pub fn style(mut self, style: ChartStyle) -> SankeyChart {
        self.style = style;
        self
    }

    /// The diagram's height in points (320 by default).
    pub fn height(mut self, height: f32) -> SankeyChart {
        self.height = if height.is_finite() { height.max(1.0) } else { 320.0 };
        self
    }

    /// The chart's elements.
    pub fn build(self) -> El {
        build(self)
    }
}

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Node(usize),
    Link(usize),
}

/// The pan and zoom: diagram points scale by `scale` about the area's top-left, then move by
/// `offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    pub(crate) scale: f32,
    pub(crate) offset: [f32; 2],
}

impl Default for Transform {
    fn default() -> Transform {
        Transform { scale: 1.0, offset: [0.0, 0.0] }
    }
}

impl Transform {
    /// A diagram point in the area's points.
    pub(crate) fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        [p[0] * self.scale + self.offset[0], p[1] * self.scale + self.offset[1]]
    }

    /// The diagram point under an area point.
    pub(crate) fn invert(&self, p: [f32; 2]) -> [f32; 2] {
        [(p[0] - self.offset[0]) / self.scale, (p[1] - self.offset[1]) / self.scale]
    }

    /// Zoomed by `factor` (2 doubles the size) about area point `at`, which stays put; the
    /// scale stays within its limits.
    pub(crate) fn zoom_at(&self, at: [f32; 2], factor: f32) -> Transform {
        if !(factor.is_finite() && factor > 0.0) {
            return *self;
        }
        let scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        let k = scale / self.scale;
        let offset = [at[0] - (at[0] - self.offset[0]) * k, at[1] - (at[1] - self.offset[1]) * k];
        Transform { scale, offset }
    }

    /// Moved by `delta` points.
    pub(crate) fn pan(&self, delta: [f32; 2]) -> Transform {
        Transform { scale: self.scale, offset: [self.offset[0] + delta[0], self.offset[1] + delta[1]] }
    }

    /// Kept so at least [`KEEP`] points of a diagram `size` points large stay inside an area of
    /// the same size.
    pub(crate) fn clamp(&self, size: [f32; 2]) -> Transform {
        let keep = |o: f32, len: f32| {
            let k = KEEP.min(len / 2.0);
            o.clamp(k - len * self.scale, len - k)
        };
        Transform { scale: self.scale, offset: [keep(self.offset[0], size[0]), keep(self.offset[1], size[1])] }
    }
}

/// Whether `p` is inside the polygon `points` (even-odd rule).
pub(crate) fn point_in_polygon(points: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    let mut j = points.len().wrapping_sub(1);
    for (i, a) in points.iter().enumerate() {
        let b = points[j];
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The pan and zoom, and the transform a drag started from.
#[derive(Default)]
struct Pan {
    view: Cell<Transform>,
    from: Cell<Option<Transform>>,
}

fn build(chart: SankeyChart) -> El {
    let style = &chart.style;
    let hover = rune::component_state::<Hover<Target>>(&format!("rune-chart-sankey/{}", chart.key));
    let pan = rune::component_state::<Pan>(&format!("rune-chart-sankey/{}", chart.key));
    let n = chart.nodes.len();
    let plot_key = format!("{}/plot", chart.key);
    let w = rune::memo::layout_size(&plot_key).map_or(480.0, |s| s[0]).max(1.0);
    let h = chart.height;
    let size = [w, h];
    let lh = line_height(style);

    let mut options = SankeyOptions::new([w, (h - 4.0).max(1.0)]);
    options.origin = [0.0, 2.0];
    options.node_width = chart.node_width;
    options.node_padding = chart.node_padding;
    options.align = chart.align;
    let layout = sankey(n, &chart.links, &options);
    let t = pan.view.get();
    let color = |i: usize| chart.nodes[i].1.unwrap_or_else(|| style.color(i));
    let dragging = pan.from.get().is_some();
    let hovered = hover.get().filter(|_| !dragging).filter(|(target, _)| match *target {
        Target::Node(i) => i < n,
        Target::Link(l) => l < layout.links.len(),
    });
    let lit = |l: usize| match hovered.map(|h| h.0) {
        None => None,
        Some(Target::Link(k)) => Some(k == l),
        Some(Target::Node(i)) => Some(layout.links[l].source == i || layout.links[l].target == i),
    };

    let bands: Vec<Vec<[f32; 2]>> = layout.links.iter().map(|l| l.band(0.5)).collect();
    let mut shape = Shape::new();
    for (l, link) in layout.links.iter().enumerate() {
        if bands[l].is_empty() {
            continue;
        }
        let alpha = match lit(l) {
            None => 0.38,
            Some(true) => 0.7,
            Some(false) => 0.1,
        };
        let points: Vec<[f32; 2]> = bands[l].iter().map(|&p| t.apply(p)).collect();
        let (x0, x1) = (t.apply([link.x0, 0.0])[0], t.apply([link.x1, 0.0])[0]);
        let gradient = Gradient::horizontal(x0, color(link.source).alpha(alpha), x1, color(link.target).alpha(alpha));
        shape = shape.fill_gradient(points, gradient);
    }
    let node_lit = |i: usize| match hovered.map(|h| h.0) {
        None => true,
        Some(Target::Node(k)) => {
            k == i
                || layout
                    .links
                    .iter()
                    .any(|l| l.width > 0.0 && (l.source == k && l.target == i || l.target == k && l.source == i))
        }
        Some(Target::Link(k)) => layout.links[k].source == i || layout.links[k].target == i,
    };
    let last = layout.nodes.iter().map(|nd| nd.layer).max().unwrap_or(0);
    let mut labels = Vec::new();
    for (i, node) in layout.nodes.iter().enumerate() {
        let [x0, y0] = t.apply([node.x0, node.y0]);
        let [x1, y1] = t.apply([node.x1, node.y1]);
        let y1 = y1.max(y0 + 1.0);
        let c = if node_lit(i) { color(i) } else { color(i).alpha(0.4) };
        shape = shape.fill(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], c);

        let (name, value) = (&chart.nodes[i].0, (chart.format)(node.value));
        let text_w = width(name, style.label) + 4.0 + width(&value, style.label);
        let left = if node.layer == last && last > 0 { x0 - LABEL_GAP - text_w } else { x1 + LABEL_GAP };
        let text = if node_lit(i) { style.text } else { style.muted };
        labels.push(
            row()
                .key(format!("node{i}"))
                .gap(4.0)
                .absolute(left, (y0 + y1) / 2.0 - lh / 2.0)
                .child(label(name, style, text).key("name"))
                .child(label(&value, style, style.text.alpha(0.6)).key("value")),
        );
    }

    let canvas = rune::canvas(shape).key("canvas").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    let cursor = if dragging { Cursor::Grabbing } else { Cursor::Default };
    let mut area = el().key(plot_key).w_full().h(h).clip_rounded(0.0).cursor(cursor).child(canvas).children(labels);
    if let Some((target, pointer)) = hovered {
        let (title, rows) = tooltip_rows(&chart, &layout, target, &color);
        area = area.child(tooltip(style, size, &title, &rows, pointer[0], pointer[1]));
    }

    let nodes: Vec<[f32; 4]> = layout.nodes.iter().map(|nd| [nd.x0, nd.y0, nd.x1, nd.y1.max(nd.y0 + 1.0)]).collect();
    let view = pan.clone();
    let area = hover.attach(area, true, move |p| {
        let p = view.view.get().invert(p);
        let near = |r: &[f32; 4]| p[0] >= r[0] - 2.0 && p[0] <= r[2] + 2.0 && p[1] >= r[1] - 2.0 && p[1] <= r[3] + 2.0;
        if let Some(i) = nodes.iter().position(near) {
            return Some(Target::Node(i));
        }
        bands.iter().rposition(|b| point_in_polygon(b, p)).map(Target::Link)
    });
    let drag = Interact { pan, hover, size };
    let (wheel, pinch) = (drag.clone(), drag.clone());
    let area = area
        .on_drag_any(move |d, tx| drag.drag(d, tx))
        .on_wheel_any(move |e, tx| wheel.wheel(e, tx))
        .on_magnify_any(move |e, _| {
            pinch.zoom([e.x, e.y], 1.0 + e.delta);
            true
        });
    col().key(chart.key.clone()).w_full().child(area)
}

/// The tooltip's title and rows for what the pointer is over.
fn tooltip_rows(
    chart: &SankeyChart,
    layout: &crate::layout::SankeyLayout,
    target: Target,
    color: &dyn Fn(usize) -> Color,
) -> (String, Vec<TooltipRow>) {
    let name = |i: usize| chart.nodes[i].0.clone();
    match target {
        Target::Link(l) => {
            let link = &layout.links[l];
            let title = format!("{} → {}", name(link.source), name(link.target));
            let row = TooltipRow { color: color(link.source), label: "Flow".into(), value: (chart.format)(link.value) };
            (title, vec![row])
        }
        Target::Node(i) => {
            let sum = |f: &dyn Fn(&crate::layout::SankeyLinkLayout) -> bool| {
                layout.links.iter().filter(|l| f(l)).map(|l| l.value).sum::<f64>()
            };
            let (inflow, outflow) = (sum(&|l| l.target == i), sum(&|l| l.source == i));
            let mut rows = Vec::new();
            if inflow > 0.0 {
                rows.push(TooltipRow { color: color(i), label: "In".into(), value: (chart.format)(inflow) });
            }
            if outflow > 0.0 || inflow <= 0.0 {
                rows.push(TooltipRow { color: color(i), label: "Out".into(), value: (chart.format)(outflow) });
            }
            (name(i), rows)
        }
    }
}

/// What the pan and zoom handlers work with.
#[derive(Clone)]
struct Interact {
    pan: Rc<Pan>,
    hover: Rc<Hover<Target>>,
    size: [f32; 2],
}

impl Interact {
    fn set(&self, t: Transform) {
        self.pan.view.set(t.clamp(self.size));
    }

    fn zoom(&self, at: [f32; 2], factor: f32) {
        self.set(self.pan.view.get().zoom_at(at, factor));
    }

    fn wheel(&self, e: WheelEvent, _: &mut Tx) -> bool {
        if (e.mods.cmd || e.mods.ctrl) && e.dy != 0.0 {
            self.zoom([e.x, e.y], (e.dy * 0.01).exp());
            return true;
        }
        false
    }

    fn drag(&self, d: &mut Drag, _: &mut Tx) {
        match d.phase {
            Phase::Down if d.clicks >= 2 => {
                self.pan.view.set(Transform::default());
                self.pan.from.set(None);
            }
            Phase::Down => {
                self.pan.from.set(Some(self.pan.view.get()));
                self.hover.item.set(None);
            }
            Phase::Move => {
                if let Some(from) = self.pan.from.get() {
                    self.set(from.pan([d.dx, d.dy]));
                }
            }
            Phase::Up => {
                if d.cancelled()
                    && let Some(from) = self.pan.from.get()
                {
                    self.pan.view.set(from);
                }
                self.pan.from.set(None);
            }
        }
    }
}

#[cfg(test)]
#[path = "sankey_tests.rs"]
mod tests;
