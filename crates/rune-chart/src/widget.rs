//! What the pie, radar, sankey and heat-map charts share: the hover they keep in component
//! state, the handlers that set it, and the default value format.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use rune::El;

use crate::TooltipRow;
use crate::overlay::{tooltip, width};
use crate::style::ChartStyle;
use crate::ticks::format_compact;

/// A value format: how a tooltip or label writes a value.
pub(crate) type Format = Rc<dyn Fn(f64) -> String>;

/// The default value format: three significant digits, thousands as `k`, millions as `M`
/// (`1234` reads `1.23k`, `3.14159` reads `3.14`, `42` reads `42`).
pub(crate) fn default_format() -> Format {
    Rc::new(|v: f64| {
        if v == 0.0 || !v.is_finite() {
            return format_compact(v, 1.0);
        }
        let step = 10f64.powf(v.abs().log10().floor() - 2.0).max(1e-6);
        format_compact(v, step)
    })
}

/// A chart's hover: what is under the pointer (`H`, an index or a pair), where the pointer is,
/// and the items its legend has hidden.
pub(crate) struct Hover<H> {
    pub(crate) item: Cell<Option<H>>,
    pub(crate) pointer: Cell<Option<[f32; 2]>>,
    pub(crate) hidden: Rc<RefCell<BTreeSet<String>>>,
}

impl<H> Default for Hover<H> {
    fn default() -> Hover<H> {
        Hover { item: Cell::new(None), pointer: Cell::new(None), hidden: Rc::default() }
    }
}

impl<H: Copy + PartialEq + 'static> Hover<H> {
    /// The hovered item and the pointer, while the pointer is over the area.
    pub(crate) fn get(&self) -> Option<(H, [f32; 2])> {
        Some((self.item.get()?, self.pointer.get()?))
    }

    /// Hover handlers for an area: `hit` finds the item under a point in the area's points.
    /// With `follow` the view re-runs as the pointer moves (a tooltip at the pointer); without
    /// it, only when the item under it changes.
    pub(crate) fn attach(self: &Rc<Self>, el: El, follow: bool, hit: impl Fn([f32; 2]) -> Option<H> + 'static) -> El {
        let (moved, left) = (self.clone(), self.clone());
        el.on_pointer_move_any(move |e, tx| {
            let item = hit([e.x, e.y]);
            if item == moved.item.get() && (!follow || item.is_none()) {
                tx.live_only();
            }
            moved.item.set(item);
            moved.pointer.set(Some([e.x, e.y]));
        })
        .on_hover_change_any(move |inside, _| {
            if !inside {
                left.item.set(None);
                left.pointer.set(None);
            }
        })
    }
}

/// The shared tooltip beside a span of the area from `left` to `right` (a cell, the whole
/// radar), centred on `y`: right of it when it fits there, else left of it, so it never covers
/// what it describes.
pub(crate) fn tooltip_beside(
    style: &ChartStyle,
    size: [f32; 2],
    title: &str,
    rows: &[TooltipRow],
    (left, right): (f32, f32),
    y: f32,
) -> El {
    // The tooltip's width, as `tooltip` sizes it, to tell whether it fits on the right.
    let font = style.label;
    let row_w = |r: &TooltipRow| 14.0 + width(&r.label, font) + 16.0 + width(&r.value, font);
    let w = rows.iter().map(row_w).fold(width(title, font), f32::max).ceil() + 20.0;
    if right + 14.0 + w <= size[0] {
        tooltip(style, size, title, rows, right, y)
    } else {
        // An area that ends at `left` puts the tooltip left of it.
        tooltip(style, [left.max(0.0), size[1]], title, rows, left, y)
    }
}

/// `a` mixed `t` of the way to `b` in Oklab, so a ramp keeps an even lightness and doesn't dip
/// through grey; `t` is clamped to `[0, 1]`.
pub(crate) fn mix_oklab(a: rune::Color, b: rune::Color, t: f32) -> rune::Color {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let (p, q) = (a.to_oklab(), b.to_oklab());
    rune::Color::from_oklab([0, 1, 2, 3].map(|i| p[i] + (q[i] - p[i]) * t))
}

#[cfg(test)]
#[path = "widget_tests.rs"]
mod tests;
