//! Builds a [`Chart`]'s elements: legend, y labels, the plot area (grid, plots, crosshair,
//! tooltip, box) and x labels.

use std::rc::Rc;

use rune::{Cursor, El, LineCap, Shape, col, el, row};

use crate::overlay::{label, legend, line_height, tooltip, width};

use super::memory::{Gesture, Interact, Memory};
use super::plot::{Frame, HoverPoint, Plot, TooltipRow, hidden_whole};
use super::{Chart, x_state};
use crate::data::union;
use crate::style::ChartStyle;
use crate::viewport::{Limits, Viewport};

/// Gap between an axis label and the plot area, and between labels.
const GAP: f32 = 8.0;

/// Pads `(a, b)` by `fraction` of its span on each side, and widens an empty one.
fn pad((a, b): (f64, f64), fraction: f64) -> (f64, f64) {
    let span = b - a;
    if span.abs() < f64::EPSILON * a.abs().max(1.0) {
        let d = if a == 0.0 { 1.0 } else { a.abs() * 0.1 };
        return (a - d, b + d);
    }
    (a - span * fraction, b + span * fraction)
}

pub(super) fn build(mut chart: Chart) -> El {
    let own = rune::component_state::<Memory>(&format!("rune-chart/{}", chart.key));
    let xs = x_state(chart.link.as_deref().unwrap_or(&chart.key));
    let hidden = own.hidden.borrow().clone();
    // Each plot's series take the palette's next colours, one per legend entry.
    let mut palette = 0;
    for plot in chart.plots.iter_mut() {
        plot.set_default_color(chart.style.color(palette));
        plot.prepare(&chart.style, palette, &hidden);
        palette += plot.legend().len();
    }
    let plots: Rc<[Box<dyn Plot>]> = std::mem::take(&mut chart.plots).into();
    let shown = || plots.iter().filter(|p| !hidden_whole(p.as_ref(), &hidden));

    // The x domain: asked for by the app, zoomed, or the data's.
    let extent = shown().filter_map(|p| p.x_extent()).reduce(|a, b| union(Some(a), Some(b)).unwrap_or(a));
    let base_x = chart.x.domain.or(extent).unwrap_or((0.0, 1.0));
    if chart.x_domain.is_some() && chart.x_domain != xs.asked.get() {
        xs.x.set(chart.x_domain);
    }
    xs.asked.set(chart.x_domain);
    let x = xs.x.get().unwrap_or(base_x);
    let x = if x.0 == x.1 { pad(x, 0.0) } else { x };

    // The y domain: zoomed, fixed, or fitted to what's in view.
    let fitted = shown().filter_map(|p| p.y_extent(x)).reduce(|a, b| union(Some(a), Some(b)).unwrap_or(a));
    let fitted = fitted.map(|(a, b)| if chart.y.zero { (a.min(0.0), b.max(0.0)) } else { (a, b) });
    let base_y = chart.y.domain.unwrap_or_else(|| {
        let (a, b) = pad(fitted.unwrap_or((0.0, 1.0)), 0.06);
        if chart.y.zero && fitted.is_some_and(|f| f.0 >= 0.0) { (a.max(0.0), b) } else { (a, b) }
    });
    let base_y = if chart.y.is_log() { (base_y.0.max(f64::MIN_POSITIVE), base_y.1) } else { base_y };
    let y = own.y.get().unwrap_or(base_y);

    let plot_key = format!("{}/plot", chart.key);
    let size = rune::memo::layout_size(&plot_key).unwrap_or([480.0, chart.height]);
    let size = [size[0].max(1.0), chart.height];
    let viewport = Viewport { x, y };
    let x_scale = chart.x.scale(x, (0.0, size[0]));
    let y_scale = chart.y.scale(y, (size[1], 0.0));
    let x_ticks = x_scale.ticks((size[0] / 90.0).max(2.0) as usize);
    let y_ticks = y_scale.ticks((size[1] / 40.0).max(2.0) as usize);
    let y_format = chart.y.formatter(&y_scale, y_ticks.len().max(2));
    let x_format = chart.x.formatter(&x_scale, x_ticks.len().max(2));
    let title = chart.title.clone().unwrap_or_else(|| chart.x.title_formatter(&x_scale));
    let frame = Frame {
        size,
        viewport,
        x: x_scale,
        y: y_scale,
        format_x: title.clone(),
        format_y: y_format.clone(),
        hover: xs.hover.get(),
    };

    let data = Viewport { x: extent.unwrap_or(base_x), y: fitted.unwrap_or(base_y) };
    let limits = Limits::around(data, 0.25).min_span(data_span(data.x) / 1e5, data_span(data.y) / 1e5);
    let interact = Interact {
        x: xs.clone(),
        own: own.clone(),
        frame: frame.clone(),
        plots: plots.clone(),
        limits,
        axes: chart.axes,
        wheel_zoom: chart.wheel_zoom,
        select: chart.select,
    };

    let y_labels: Vec<(f32, String)> = y_ticks.iter().map(|&t| (frame.y.map(t), y_format(t))).collect();
    let gutter = match chart.gutter {
        Some(gutter) => gutter,
        None if chart.y.visible => {
            y_labels.iter().map(|(_, s)| width(s, chart.style.label)).fold(0.0, f32::max).ceil() + GAP
        }
        None => 0.0,
    };
    let shape = draw(&chart, &frame, &plots, &hidden, &x_ticks, &y_ticks, &xs, &own);
    let plot_area = plot_area(&chart, &frame, &plots, &hidden, shape, &plot_key, &own, xs.hover.get(), interact);
    let lh = line_height(&chart.style);
    let y_axis = el().key("y-labels").w(gutter).h(size[1]).children(
        chart.y.visible.then_some(y_labels).into_iter().flatten().enumerate().map(|(i, (py, s))| {
            let top = (py - lh / 2.0).clamp(-lh / 2.0, size[1] - lh / 2.0);
            label(&s, &chart.style, chart.style.muted)
                .key(format!("y{i}"))
                .absolute(gutter - GAP - width(&s, chart.style.label), top)
        }),
    );
    let mut out = col().key(chart.key.clone()).w_full().gap(6.0);
    let items: Vec<(String, rune::Color)> = plots.iter().flat_map(|p| p.legend()).collect();
    if chart.legend && items.len() > 1 {
        out = out.child(legend(&chart.style, &items, &own.hidden));
    }
    out = out.child(row().key("body").w_full().h(size[1]).child(y_axis).child(plot_area));
    if chart.x.visible {
        out = out.child(x_axis(&chart.style, &frame, &x_ticks, &x_format, gutter, lh));
    }
    out
}

fn data_span((a, b): (f64, f64)) -> f64 {
    (b - a).abs().max(f64::MIN_POSITIVE)
}

/// Grid, plots, a selected range and the crosshair, in the plot area's points.
#[allow(clippy::too_many_arguments)]
fn draw(
    chart: &Chart,
    frame: &Frame,
    plots: &[Box<dyn Plot>],
    hidden: &std::collections::BTreeSet<String>,
    x_ticks: &[f64],
    y_ticks: &[f64],
    xs: &super::memory::XMemory,
    own: &Memory,
) -> Shape {
    let [w, h] = frame.size;
    let style = &chart.style;
    let mut shape = Shape::new();
    if chart.y.grid {
        for &t in y_ticks {
            let y = frame.y.map(t).round() + 0.5;
            shape = shape.line([0.0, y], [w, y], 1.0, LineCap::Butt, style.grid);
        }
    }
    if chart.x.grid && chart.x.visible {
        for &t in x_ticks {
            let x = frame.x.map(t).round() + 0.5;
            shape = shape.line([x, 0.0], [x, h], 1.0, LineCap::Butt, style.grid.alpha(style.grid.a * 0.6));
        }
    }
    if let Some((a, b)) = xs.selection.get() {
        let (l, r) = (frame.x.map(a), frame.x.map(b));
        shape = shape.fill(vec![[l, 0.0], [r, 0.0], [r, h], [l, h]], style.selection);
    }
    for plot in plots.iter().filter(|p| !hidden_whole(p.as_ref(), hidden)) {
        shape = plot.draw(frame, shape);
    }
    let pointer = own.pointer.get().unwrap_or([0.0, 0.0]);
    if let Some(hx) = xs.hover.get().filter(|&hx| frame.x_visible(hx)) {
        let x = frame.x.map(hx).round() + 0.5;
        shape = shape.stroke_dashed(vec![[x, 0.0], [x, h]], 1.0, 3.0, 3.0, LineCap::Butt, style.crosshair);
        for point in hover_points(plots, hidden, frame, hx, pointer) {
            if let Some(dot) = point.dot {
                shape = shape.dot(dot, 4.5, style.tooltip).dot(dot, 3.0, point.row.color);
            }
        }
    }
    shape
}

/// The plot area: its clipped drawing, the tooltip and a box being drawn, with the handlers.
#[allow(clippy::too_many_arguments)]
fn plot_area(
    chart: &Chart,
    frame: &Frame,
    plots: &[Box<dyn Plot>],
    hidden: &std::collections::BTreeSet<String>,
    shape: Shape,
    key: &str,
    own: &Memory,
    hover: Option<f64>,
    interact: Interact,
) -> El {
    let style = &chart.style;
    let gesture = own.gesture.get();
    let canvas = rune::canvas(shape).key("canvas").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).clip_rounded(0.0);
    let mut area = el()
        .key(key.to_owned())
        .grow(1.0)
        .h(frame.size[1])
        .cursor(if matches!(gesture, Gesture::Pan(_)) { Cursor::Grabbing } else { Cursor::Crosshair })
        .child(canvas);
    let lh = line_height(style);
    let labels = plots.iter().filter(|p| !hidden_whole(p.as_ref(), hidden)).flat_map(|p| p.labels(frame));
    for (i, l) in labels.enumerate() {
        // On a translucent pill, so a label stays readable over the data.
        let (w, h) = (width(&l.text, style.label) + 8.0, lh + 2.0);
        let left = (l.at[0] - l.anchor[0] * w).clamp(0.0, (frame.size[0] - w).max(0.0));
        let top = (l.at[1] - l.anchor[1] * h).clamp(0.0, (frame.size[1] - h).max(0.0));
        let text = label(&l.text, style, l.color).key("text");
        let pill = el().key(format!("label{i}")).absolute(left, top).px(4.0).py(1.0).radius(3.0);
        area = area.child(pill.fill(style.tooltip.alpha(0.75)).child(text));
    }
    if let Gesture::Box { from, to, select } = gesture {
        let (l, r) = (from[0].min(to[0]), from[0].max(to[0]));
        let (t, b) = if select { (0.0, frame.size[1]) } else { (from[1].min(to[1]), from[1].max(to[1])) };
        area = area.child(
            el().key("box")
                .absolute(l, t)
                .size(r - l, b - t)
                .fill(style.selection)
                .border(1.0, style.selection.alpha(0.8)),
        );
    }
    let pointer = own.pointer.get();
    if let (Some(hx), Some(pointer)) = (hover.filter(|&hx| frame.x_visible(hx)), pointer)
        && !matches!(gesture, Gesture::Pan(_) | Gesture::Box { .. })
    {
        let points = hover_points(plots, hidden, frame, hx, pointer);
        // The title reads the shown point's x: where the crosshair snapped, or the scatter point
        // under a pointer that no plot snaps.
        let title_x = points.first().map_or(hx, |p| p.x);
        let rows: Vec<TooltipRow> = points.into_iter().map(|p| p.row).collect();
        if !rows.is_empty() {
            let title = (frame.format_x)(title_x);
            area = area.child(tooltip(style, frame.size, &title, &rows, frame.x.map(title_x), pointer[1]));
        }
    }
    interact.attach(area)
}

/// What the shown plots show at data `x` with the pointer at `pointer`.
fn hover_points(
    plots: &[Box<dyn Plot>],
    hidden: &std::collections::BTreeSet<String>,
    frame: &Frame,
    x: f64,
    pointer: [f32; 2],
) -> Vec<HoverPoint> {
    plots.iter().filter(|p| !hidden_whole(p.as_ref(), hidden)).flat_map(|p| p.hover_points(frame, x, pointer)).collect()
}

/// The x tick labels under the plot area, thinned so none overlap.
fn x_axis(
    style: &ChartStyle,
    frame: &Frame,
    ticks: &[f64],
    format: &Rc<dyn Fn(f64) -> String>,
    gutter: f32,
    lh: f32,
) -> El {
    let mut right = f32::NEG_INFINITY;
    let mut labels = Vec::new();
    for (i, &t) in ticks.iter().enumerate() {
        let s = format(t);
        let w = width(&s, style.label);
        let left = (frame.x.map(t) - w / 2.0).clamp(0.0, (frame.size[0] - w).max(0.0));
        if left < right + GAP {
            continue;
        }
        right = left + w;
        labels.push(label(&s, style, style.muted).key(format!("x{i}")).absolute(gutter + left, 0.0));
    }
    el().key("x-labels").w_full().h(lh).children(labels)
}
