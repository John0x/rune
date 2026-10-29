//! Selecting blocks with a rectangle, as Notion does: a drag that starts outside the text
//! (between or beside the blocks, below them, or in the page around the editor with
//! [`selection_area`]) draws a rectangle, and the blocks it crosses are selected whole. A
//! click there without a drag puts the caret in the block beside it, or writes below the
//! last one.

use std::rc::Rc;

use rune::{Color, Cx, Drag, El, Phase, Tx, col, el};

use super::{Blocks, Kind, keys::field_key, view::Wire};
use crate::Kit;

/// How far the pointer travels before a press is a drag.
const SLOP: f32 = 4.0;

/// A rectangle being dragged, from `start` to `now` (window points). The start's `y` counts
/// from the top of the rows, so it stays on the blocks while the view scrolls; `top` is where
/// that was when the drag last moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Marquee {
    start: [f32; 2],
    now: [f32; 2],
    top: f32,
}

impl Marquee {
    fn start(&self) -> [f32; 2] {
        [self.start[0], self.start[1] + self.top]
    }

    fn moved(&self) -> bool {
        let start = self.start();
        (self.now[0] - start[0]).abs() > SLOP || (self.now[1] - start[1]).abs() > SLOP
    }

    /// `[left, top, right, bottom]`.
    fn rect(&self) -> [f32; 4] {
        let [a, b] = [self.start(), self.now];
        [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
    }
}

/// The rows' heights (0 for blocks a closed toggle hides) and where the first starts on
/// screen, once laid out.
#[derive(Clone, Debug)]
pub(super) struct Rows {
    pub(super) heights: Rc<Vec<f32>>,
    pub(super) top: Option<f32>,
    /// Whether every shown row has been laid out.
    pub(super) complete: bool,
}

impl Rows {
    /// The rows of the editor keyed `editor`, as the last layout placed them.
    pub(super) fn read<S>(cx: &Cx<S>, editor: &str, doc: &Blocks) -> Rows {
        let hidden = doc.hidden();
        let height = |b: &super::Block| cx.layout_size(&super::view::row_key(editor, b.id)).map_or(0.0, |s| s[1]);
        let heights: Vec<f32> =
            doc.list.iter().zip(&hidden).map(|(b, hidden)| if *hidden { 0.0 } else { height(b) }).collect();
        let complete = heights.iter().zip(&hidden).all(|(h, hidden)| *hidden || *h > 0.0);
        Rows { heights: Rc::new(heights), top: cx.layout_with(&format!("{editor}-rows"), |r| r[1]), complete }
    }

    fn laid_out(&self) -> Option<f32> {
        self.top.filter(|_| self.complete)
    }

    /// The row at window `y`, when the rows have been laid out.
    pub(super) fn at(&self, y: f32) -> Option<usize> {
        let mut bottom = self.laid_out()?;
        for (i, h) in self.heights.iter().enumerate() {
            bottom += h;
            if y < bottom && *h > 0.0 {
                return Some(i);
            }
        }
        Some(self.heights.len().saturating_sub(1))
    }

    /// The first and last rows between window `top` and `bottom`.
    fn between(&self, top: f32, bottom: f32) -> Option<(usize, usize)> {
        let mut y = self.laid_out()?;
        let mut hit: Option<(usize, usize)> = None;
        for (i, h) in self.heights.iter().enumerate() {
            if y < bottom && y + h > top {
                hit = Some((hit.map_or(i, |(first, _)| first), i));
            }
            y += h;
        }
        hit
    }

    /// The bottom of the last row.
    fn bottom(&self) -> Option<f32> {
        Some(self.laid_out()? + self.heights.iter().sum::<f32>())
    }
}

/// What a press outside the text did once released.
enum Released {
    Dragged,
    /// A click below the blocks.
    Below,
    /// A click beside block `i`.
    Beside(usize),
    Nowhere,
}

impl Blocks {
    /// A drag outside the text at window point `at`.
    fn marquee(&mut self, phase: Phase, at: [f32; 2], rows: &Rows) -> Option<Released> {
        let top = rows.top.unwrap_or(0.0);
        match phase {
            Phase::Down => {
                self.marquee = Some(Marquee { start: [at[0], at[1] - top], now: at, top });
                self.selection = None;
                self.menu = None;
                None
            }
            Phase::Move => {
                let m = self.marquee.as_mut()?;
                (m.now, m.top) = (at, top);
                if m.moved() {
                    let [_, top, _, bottom] = m.rect();
                    match rows.between(top, bottom) {
                        Some((first, last)) => self.select(self.list[first].id, self.list[last].id),
                        None => self.selection = None,
                    }
                }
                None
            }
            Phase::Up => {
                let m = self.marquee.take()?;
                if m.moved() {
                    return Some(Released::Dragged);
                }
                Some(match rows.bottom() {
                    Some(bottom) if at[1] >= bottom => Released::Below,
                    Some(_) => rows.at(at[1]).map_or(Released::Nowhere, Released::Beside),
                    None => Released::Nowhere,
                })
            }
        }
    }

    /// A click below the blocks: focuses the last one when it's an empty paragraph, else adds
    /// one. Whether the document changed.
    fn write_below(&mut self) -> bool {
        let last = self.list.len() - 1;
        let b = &self.list[last];
        if b.kind == Kind::Paragraph && b.edit.text.is_empty() {
            self.focus(last, 0);
            return false;
        }
        self.insert(last + 1, Kind::Paragraph, 0, "", Vec::new());
        true
    }
}

/// The drag handler of an element at window point `origin` (its own `[x, y]`).
pub(super) fn handler<S: 'static>(
    wire: &Wire<S>,
    rows: Rows,
    origin: Option<[f32; 2]>,
) -> impl Fn(&mut S, &mut Drag, &mut Tx) + 'static {
    let wire = wire.clone();
    move |s: &mut S, d: &mut Drag, tx: &mut Tx| {
        let Some([ox, oy]) = origin else { return };
        let at = [ox + d.x, oy + d.y];
        let phase = if d.cancelled() { Phase::Up } else { d.phase };
        let released = (wire.access)(s).marquee(phase, at, &rows);
        match released {
            Some(Released::Below) => wire.edit(s, tx, Blocks::write_below),
            Some(Released::Beside(i)) => {
                let doc = (wire.access)(s);
                if let Some(b) = doc.list.get(i).filter(|b| b.kind.has_text()) {
                    tx.place_caret(field_key(&wire.editor, b.id), at[0], at[1]);
                }
            }
            _ => {}
        }
    }
}

/// The rectangle being dragged, drawn in an element at window point `origin`.
pub(super) fn rectangle(k: &Kit, doc: &Blocks, origin: Option<[f32; 2]>) -> Option<El> {
    let (m, [ox, oy]) = (doc.marquee.filter(Marquee::moved)?, origin?);
    let [l, t, r, b] = m.rect();
    let accent = k.c.text_accent;
    Some(
        el().key("marquee")
            .absolute(l - ox, t - oy)
            .size(r - l, b - t)
            .radius(3.0)
            .fill(accent.alpha(0.12))
            .border(1.0, accent.alpha(0.45))
            .opacity(1.0),
    )
}

/// `area` (the page around the editor keyed `editor`: its margins) lets a drag that starts
/// in it select `doc`'s blocks with a rectangle too, and a click in it put the caret in the
/// block beside it.
pub fn selection_area<S: 'static>(
    editor: &str,
    doc: &Blocks,
    cx: &Cx<S>,
    access: impl for<'b> Fn(&'b mut S) -> &'b mut Blocks + 'static,
    area: El,
) -> El {
    let key = format!("{editor}-area");
    let origin = cx.layout_with(&key, |r| [r[0], r[1]]);
    let wire = Wire { access: Rc::new(access), change: None, editor: editor.into() };
    let rows = Rows::read(cx, editor, doc);
    col()
        .key(key)
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .fill(Color::TRANSPARENT)
        .child(area)
        .drag_autoscroll()
        .on_drag(handler(&wire, rows, origin))
}
