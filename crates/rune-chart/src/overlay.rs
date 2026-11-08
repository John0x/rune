//! What every chart shares on top of its drawing: label text, the tooltip and the legend.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use rune::{Color, Cursor, El, Phase, TextStyle, col, el, row, text};

use crate::chart::TooltipRow;
use crate::style::ChartStyle;

/// A label's width in points.
pub(crate) fn width(s: &str, style: TextStyle) -> f32 {
    rune::text::measure(s, &[], style, None).0
}

/// A label in the style's type.
pub(crate) fn label(s: &str, style: &ChartStyle, color: Color) -> El {
    text(s.to_owned()).font(style.label).color(color)
}

/// The style's label line height.
pub(crate) fn line_height(style: &ChartStyle) -> f32 {
    style.label.line_height.max(style.label.size * 1.3)
}

/// A tooltip for an area of `size` points: `title` over `rows` (swatch, label, value), beside
/// `x` (right of it, or left near the right edge) and centred on `y`, kept inside the area.
pub(crate) fn tooltip(style: &ChartStyle, size: [f32; 2], title: &str, rows: &[TooltipRow], x: f32, y: f32) -> El {
    let font = style.label;
    let lh = line_height(style);
    let row_w = |r: &TooltipRow| 14.0 + width(&r.label, font) + 16.0 + width(&r.value, font);
    let w = rows.iter().map(row_w).fold(width(title, font), f32::max).ceil() + 20.0;
    let h = lh * (rows.len() + 1) as f32 + 4.0 * rows.len() as f32 + 16.0;
    let left = if x + 14.0 + w <= size[0] || x - 14.0 - w < 0.0 { x + 14.0 } else { x - 14.0 - w };
    let top = (y - h / 2.0).clamp(0.0, (size[1] - h).max(0.0));
    let line = |i: usize, r: &TooltipRow| {
        row()
            .key(format!("row{i}"))
            .gap(6.0)
            .items_center()
            .child(el().key("swatch").size(8.0, 8.0).radius(4.0).fill(r.color))
            .child(label(&r.label, style, style.tooltip_text.alpha(0.75)).key("name").grow(1.0))
            .child(label(&r.value, style, style.tooltip_text).key("value"))
    };
    col()
        .key("tooltip")
        .absolute(left, top)
        .w(w)
        .padding(8.0)
        .gap(4.0)
        .radius(6.0)
        .fill(style.tooltip)
        .border(1.0, style.tooltip_border)
        .raise()
        .child(label(title, style, style.tooltip_text).key("title"))
        .children(rows.iter().enumerate().map(|(i, r)| line(i, r)))
}

/// A legend: a swatch and name per item (`(name, colour)`); a click hides or shows one, in
/// `hidden`.
pub(crate) fn legend(style: &ChartStyle, items: &[(String, Color)], hidden: &Rc<RefCell<BTreeSet<String>>>) -> El {
    let shown = hidden.borrow().clone();
    let items = items.iter().enumerate().map(|(i, (name, color))| {
        let off = shown.contains(name);
        let (toggled, hidden) = (name.clone(), hidden.clone());
        row()
            .key(format!("legend{i}"))
            .gap(6.0)
            .items_center()
            .cursor(Cursor::Pointer)
            .opacity(if off { 0.4 } else { 1.0 })
            .child(el().key("swatch").size(12.0, 3.0).radius(1.5).fill(*color))
            .child(label(name, style, style.text).key("name"))
            .on_drag_any(move |d, _| {
                if d.phase == Phase::Up && !d.cancelled() {
                    let mut hidden = hidden.borrow_mut();
                    if !hidden.remove(&toggled) {
                        hidden.insert(toggled.clone());
                    }
                }
            })
    });
    row().key("legend").gap(16.0).flex_wrap().children(items)
}
