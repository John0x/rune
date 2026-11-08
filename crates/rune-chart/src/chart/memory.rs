//! What a chart remembers between view runs (its zoom, the hovered x, a drag in progress, the
//! series hidden from the legend) and the handlers that change it.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use rune::{Drag, El, Phase, Tx};

use super::plot::{Frame, Plot, hidden_whole};
use crate::viewport::{Axes, Limits, Viewport};

/// A drag over the plot area.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Gesture {
    #[default]
    None,
    /// Panning from the viewport the drag started on.
    Pan(Viewport),
    /// A box zoom (shift-drag) or a range selection (option-drag) from `from` to `to`.
    Box { from: [f32; 2], to: [f32; 2], select: bool },
}

/// The x side of a chart's state, shared by charts linked to one x ([`Chart::link`](crate::Chart::link)).
#[derive(Default)]
pub(crate) struct XMemory {
    /// The zoomed x domain; `None` while it follows the data.
    pub(crate) x: Cell<Option<(f64, f64)>>,
    /// The x the crosshair snapped to.
    pub(crate) hover: Cell<Option<f64>>,
    /// The x domain the app last asked for ([`Chart::x_domain`](crate::Chart::x_domain)).
    pub(crate) asked: Cell<Option<(f64, f64)>>,
    /// The selected x range (option-drag).
    pub(crate) selection: Cell<Option<(f64, f64)>>,
}

/// One chart's own state.
#[derive(Default)]
pub(crate) struct Memory {
    /// The zoomed y domain; `None` while it fits what's in view.
    pub(crate) y: Cell<Option<(f64, f64)>>,
    /// The pointer over the plot area, while it is.
    pub(crate) pointer: Cell<Option<[f32; 2]>>,
    pub(crate) gesture: Cell<Gesture>,
    /// The names of the series the legend has hidden.
    pub(crate) hidden: Rc<RefCell<BTreeSet<String>>>,
}

/// What the plot area's handlers work with.
#[derive(Clone)]
pub(crate) struct Interact {
    pub(crate) x: Rc<XMemory>,
    pub(crate) own: Rc<Memory>,
    pub(crate) frame: Frame,
    pub(crate) plots: Rc<[Box<dyn Plot>]>,
    pub(crate) limits: Limits,
    /// Which axes zoom and pan change.
    pub(crate) axes: Axes,
    /// Whether a plain wheel zooms (else it needs ⌘ or Ctrl, so the page scrolls).
    pub(crate) wheel_zoom: bool,
    pub(crate) select: bool,
}

impl Interact {
    fn set(&self, view: Viewport) {
        let view = view.clamp(&self.limits);
        if matches!(self.axes, Axes::X | Axes::Both) {
            self.x.x.set(Some(view.x));
        }
        if matches!(self.axes, Axes::Y | Axes::Both) {
            self.own.y.set(Some(view.y));
        }
    }

    fn reset(&self) {
        self.x.x.set(None);
        self.own.y.set(None);
        self.x.selection.set(None);
    }

    /// The x the crosshair snaps to near pixel `px`: the nearest data x of any plot, or the
    /// pointer's own x when no plot snaps but one follows the pointer (scatter points).
    fn snap(&self, px: f32) -> Option<f64> {
        let x = self.frame.x.invert(px);
        let hidden = self.own.hidden.borrow();
        let shown = || self.plots.iter().filter(|p| !hidden_whole(p.as_ref(), &hidden));
        shown()
            .filter_map(|p| p.snap(x))
            .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
            .or_else(|| shown().any(|p| p.follows_pointer()).then_some(x))
    }

    fn zoom_at(&self, px: [f32; 2], factor: f64) {
        let anchor = self.frame.viewport.point_at(px, self.frame.size);
        self.set(self.frame.viewport.zoom(anchor, factor, self.axes));
    }

    /// The plot area's handlers: hover, wheel and pinch zoom, drag to pan, shift-drag to box
    /// zoom, option-drag to select, double click to reset.
    pub(crate) fn attach(self, el: El) -> El {
        let (hover, leave, wheel, pinch, drag) = (self.clone(), self.clone(), self.clone(), self.clone(), self);
        el.on_pointer_move_any(move |e, tx| {
            hover.own.pointer.set(Some([e.x, e.y]));
            let snapped = hover.snap(e.x);
            // The view shows the snapped x, not the pointer: an unchanged x needs no new view,
            // unless a plot's hover follows the pointer itself (scatter points).
            if snapped == hover.x.hover.get() && !hover.plots.iter().any(|p| p.follows_pointer()) {
                tx.live_only();
            }
            hover.x.hover.set(snapped);
        })
        .on_hover_change_any(move |inside, _| {
            if !inside {
                leave.own.pointer.set(None);
                leave.x.hover.set(None);
            }
        })
        .on_wheel_any(move |e, tx| wheel.wheel(e, tx))
        .on_magnify_any(move |e, _| {
            pinch.zoom_at([e.x, e.y], 1.0 + e.delta as f64);
            true
        })
        .on_drag_any(move |d, tx| drag.drag(d, tx))
    }

    fn wheel(&self, e: rune::WheelEvent, _: &mut Tx) -> bool {
        let zoom = self.wheel_zoom || e.mods.cmd || e.mods.ctrl;
        if zoom && e.dy != 0.0 {
            self.zoom_at([e.x, e.y], (e.dy as f64 * 0.01).exp());
            return true;
        }
        // A sideways swipe pans; a plain vertical wheel scrolls the page.
        if e.dx.abs() > e.dy.abs() {
            self.set(self.frame.viewport.pan([e.dx, 0.0], self.frame.size, Axes::X));
            return true;
        }
        false
    }

    fn drag(&self, d: &mut Drag, tx: &mut Tx) {
        let mods = tx.mods();
        match d.phase {
            Phase::Down if d.clicks >= 2 => {
                self.reset();
                self.own.gesture.set(Gesture::None);
            }
            Phase::Down => {
                let from = [d.start_x, d.start_y];
                self.own.gesture.set(if mods.shift || (mods.alt && self.select) {
                    Gesture::Box { from, to: from, select: !mods.shift }
                } else {
                    Gesture::Pan(self.frame.viewport)
                });
            }
            Phase::Move => match self.own.gesture.get() {
                Gesture::Pan(start) => self.set(start.pan([d.dx, d.dy], self.frame.size, self.axes)),
                Gesture::Box { from, select, .. } => {
                    self.own.gesture.set(Gesture::Box { from, to: [d.x, d.y], select })
                }
                Gesture::None => {}
            },
            Phase::Up => {
                if let Gesture::Box { from, to, select } = self.own.gesture.get()
                    && !d.cancelled()
                {
                    if select {
                        let (a, b) = (self.frame.x.invert(from[0]), self.frame.x.invert(to[0]));
                        self.x.selection.set((a != b).then(|| (a.min(b), a.max(b))));
                    } else {
                        self.set(self.frame.viewport.box_zoom(from, to, self.frame.size, self.axes));
                    }
                }
                self.own.gesture.set(Gesture::None);
            }
        }
    }
}
