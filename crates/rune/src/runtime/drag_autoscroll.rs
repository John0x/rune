//! Scrolling under a held drag: an [`El::drag_autoscroll`](crate::El::drag_autoscroll)
//! element's drag near the top or bottom edge of its scroll viewport (around it, or inside it
//! under the pointer) scrolls it, as a dragged row in a list or a text selection does in AppKit.

use super::*;

/// How far from the viewport's edge the pointer starts scrolling it (points).
const BAND: f32 = 48.0;
/// The speed at the edge itself (points per second); it falls to zero across the band.
const SPEED: f32 = 1200.0;

impl<S: 'static> Runtime<S> {
    /// Scrolls the viewport under a held autoscroll drag by `dt` seconds' worth, and gives the
    /// drag a Move from where the pointer now is over the content. Whether it scrolled.
    pub(super) fn step_drag_autoscroll(&mut self, dt: f32) -> bool {
        let (Some((path, _)), Some((x, y))) = (&self.capture, self.pointer) else { return false };
        if dt <= 0.0 || self.input_click.captured {
            return false;
        }
        let Some(i) = self.nodes.iter().position(|n| &n.path == path && n.drag_autoscroll && n.on_drag.is_some()) else {
            return false;
        };
        let scrolls_y = |p: &usize| self.nodes[*p].scroll.is_some() && self.nodes[*p].scroll_axes.y();
        let inside = |p: usize| std::iter::successors(self.nodes[p].parent, |&q| self.nodes[q].parent).any(|q| q == i);
        let across = |p: &usize| self.drawn.get(*p).is_some_and(|r| x >= r[0] && x <= r[0] + r[2]);
        // The viewport around the element or, for one around a page, the innermost inside it
        // under the pointer.
        let viewport = std::iter::successors(self.nodes[i].parent, |&p| self.nodes[p].parent)
            .find(scrolls_y)
            .or_else(|| (i + 1..self.nodes.len()).rev().filter(scrolls_y).filter(across).find(|&p| inside(p)));
        let Some((v, rect)) = viewport.and_then(|v| Some((v, *self.drawn.get(v)?))) else { return false };
        let (top, bottom) = (rect[1], rect[1] + rect[3]);
        let band = BAND.min(rect[3] / 4.0);
        let speed = if y < top + band {
            -SPEED * ((top + band - y) / band).min(1.0)
        } else if y > bottom - band {
            SPEED * ((y - (bottom - band)) / band).min(1.0)
        } else {
            return false;
        };
        let Some(st) = self.scrolls.get_mut(&self.nodes[v].id) else { return false };
        let before = st.offset;
        st.offset = (st.offset + f64::from(speed * dt)).clamp(0.0, st.max);
        st.v = 0.0;
        st.glide = None;
        st.pinned = false;
        if st.offset == before {
            return false;
        }
        self.move_pointer(x, y, true);
        true
    }
}
