//! The handles a hovered block shows in the gutter, as Notion's: `+` adds a block below (with
//! the slash menu), and ⋮⋮ drags the block — or the selection it's part of — with the blocks
//! nested under it: a faded copy follows the pointer, a line shows where they'll land, and a
//! sideways drag nests them. A click on ⋮⋮ selects the block and opens its menu: turn into
//! another kind, color, duplicate, copy as Markdown, delete.

use std::{ops::Range, rc::Rc};

use rune::{Color, Cursor, Drag, El, Icon, Phase, Tx, el, icon, row};

use super::toolbar::{ColorItem, color_items, color_of};
use super::{Block, Blocks, Kind, markdown, menu::CHOICES, view::Wire};
use crate::{Kit, Menu, MenuEntry, MenuItem};

/// Width of a nesting level, which a sideways drag steps by.
pub(super) const INDENT: f32 = 24.0;
/// Width of the gutter left of the blocks, where a hovered block's handles show.
pub const GUTTER: f32 = 48.0;
/// How far a press on ⋮⋮ travels before it drags.
const SLOP: f32 = 4.0;

static HANDLE: Icon = Icon {
    name: "blocks-handle",
    view: 16.0,
    body: r#"<g fill="currentColor"><circle cx="6" cy="3.5" r="1.25"/><circle cx="10" cy="3.5" r="1.25"/><circle cx="6" cy="8" r="1.25"/><circle cx="10" cy="8" r="1.25"/><circle cx="6" cy="12.5" r="1.25"/><circle cx="10" cy="12.5" r="1.25"/></g>"#,
};
static PLUS: Icon = Icon {
    name: "blocks-plus",
    view: 16.0,
    body: r#"<path d="M8 3V13M3 8H13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
};

/// Blocks being dragged by a handle: the first and last moved (the last with the blocks
/// nested under it), how far the pointer moved over the content (scrolling included), and
/// where they would land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BlockDrag {
    first: u64,
    last: u64,
    dx: f32,
    dy: f32,
    /// The index they would land before, and how many levels deeper.
    to: Option<(usize, i8)>,
}

impl BlockDrag {
    fn moved(&self) -> bool {
        self.dx.abs() > SLOP || self.dy.abs() > SLOP
    }
}

/// The handles left of block `b`; `anchor` is where its menu opens (window points).
pub(super) fn gutter<S: 'static>(
    line: f32,
    color: Color,
    shown: bool,
    b: &Block,
    wire: Option<&Wire<S>>,
    heights: &Rc<Vec<f32>>,
    anchor: Option<[f32; 2]>,
) -> El {
    let g = row().key("gutter").w(GUTTER).h(line).pr(4.0).justify_end().center_y().gap(2.0).opacity(if shown {
        1.0
    } else {
        0.0
    });
    let Some(wire) = wire else { return g };
    let id = b.id;
    let button = |key: &'static str, glyph: &'static Icon| {
        el().key(key).size(20.0, 24.0).center().radius(4.0).child(icon(glyph, 15.0).color(color))
    };
    let add = {
        let wire = wire.clone();
        button("add", &PLUS).hit().cursor(Cursor::Pointer).on_press_tx(move |s: &mut S, tx| {
            wire.edit(s, tx, |doc| doc.add_below(id));
        })
    };
    let (wire, heights) = (wire.clone(), heights.clone());
    let drag = button("drag", &HANDLE).hit().cursor(Cursor::Grab).drag_autoscroll().on_drag(
        move |s: &mut S, d: &mut Drag, tx| {
            let doc = (wire.access)(s);
            match d.phase {
                Phase::Down => doc.grab(id),
                Phase::Move => {
                    if let Some(mut g) = doc.drag {
                        // The handle stays put in the content, so this counts scrolling too.
                        (g.dx, g.dy) = (d.dx, d.y - d.start_y);
                        g.to = g.moved().then(|| doc.drop_target(g, &heights)).flatten();
                        doc.drag = Some(g);
                    }
                }
                Phase::Up => {
                    let Some(g) = doc.drag.take() else { return };
                    if d.cancelled() {
                        return;
                    }
                    if !g.moved() {
                        doc.select(id, id);
                        doc.block_menu = anchor.map(|at| (id, at));
                    } else if doc.drop(g) {
                        doc.commit(None);
                        wire.changed(s, tx);
                    }
                }
            }
        },
    );
    g.child(add).child(drag)
}

/// The menu of the block whose ⋮⋮ was clicked, when open.
pub(super) fn block_menu<S: 'static>(k: &Kit, doc: &Blocks, wire: &Wire<S>, layer: &mut Vec<El>) {
    let Some((id, [x, y])) = doc.block_menu else { return };
    let Some(i) = doc.index(id) else { return };
    let kind = doc.list[i].kind.clone();
    let act = |label: &str, shortcut: &str, f: fn(&mut Blocks, &mut Tx) -> bool| {
        let wire = wire.clone();
        MenuItem::new(label.to_owned()).shortcut(shortcut.to_owned()).on_select_tx(move |s: &mut S, tx| {
            let doc = (wire.access)(s);
            doc.block_menu = None;
            if f(doc, tx) {
                doc.commit(None);
                wire.changed(s, tx);
            }
        })
    };
    let colors = {
        let wire = wire.clone();
        MenuItem::new("Color").submenu(block_colors(k, doc, &wire, i), doc.block_colors).on_select(move |s: &mut S| {
            let doc = (wire.access)(s);
            doc.block_colors = !doc.block_colors;
        })
    };
    let mut menu = Menu::new(format!("{}-block-menu", wire.editor))
        .open(true)
        .at(x, y)
        .min_width(220.0)
        .on_close({
            let wire = wire.clone();
            move |s: &mut S| (wire.access)(s).block_menu = None
        })
        .item(act("Duplicate", "⌘D", |doc, _| doc.selected().map(|range| doc.duplicate(range)).is_some()))
        .item(act("Copy", "⌘C", |doc, tx| {
            if let Some(range) = doc.selected() {
                tx.copy(markdown::write(&doc.list[range]));
            }
            false
        }))
        .item(act("Delete", "⌫", |doc, _| {
            let range = doc.selected();
            doc.selection = None;
            range.map(|range| doc.delete_range(range)).is_some()
        }))
        .item(colors)
        .separator()
        .group_label("Turn into");
    for choice in &CHOICES {
        let (target, wire) = (choice.kind(), wire.clone());
        let current = target.same_kind(&kind);
        menu = menu.item(MenuItem::new(target.label()).checked(current).on_select_tx(move |s: &mut S, tx| {
            let doc = (wire.access)(s);
            doc.block_menu = None;
            let Some(range) = doc.selected() else { return };
            for i in range.rev() {
                if doc.list[i].kind.has_text() || target == Kind::Divider {
                    doc.turn_into(i, target.clone());
                }
            }
            doc.selection = None;
            doc.commit(None);
            wire.changed(s, tx);
        }));
    }
    let _ = menu.build(k, layer);
}

/// The block menu's colors, checked as block `i`'s whole text is colored.
fn block_colors<S: 'static>(k: &Kit, doc: &Blocks, wire: &Wire<S>, i: usize) -> Vec<MenuEntry> {
    let b = &doc.list[i];
    let whole = |background| {
        b.edit
            .marks()
            .iter()
            .find(|m| m.start == 0 && m.end == b.edit.text.len())
            .and_then(|m| color_of(&m.mark, background))
    };
    color_items(k, wire, whole(false), whole(true))
        .into_iter()
        .map(|item| match item {
            ColorItem::Label(l) => MenuEntry::Label(l.to_owned()),
            ColorItem::Item(item) => MenuEntry::item(*item),
        })
        .collect()
}

impl Kind {
    /// Whether `self` and `other` are the same kind, whatever their details (a checked to-do
    /// is a to-do).
    pub(super) fn same_kind(&self, other: &Kind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && !matches!((self, other), (Kind::Heading(a), Kind::Heading(b)) if a != b)
    }
}

impl Blocks {
    /// A press on block `id`'s ⋮⋮: it drags the selection when the block is part of it, else
    /// the block, which it selects.
    fn grab(&mut self, id: u64) {
        let Some(i) = self.index(id) else { return };
        let range = self.selected().filter(|range| range.contains(&i)).unwrap_or_else(|| {
            self.select(id, id);
            i..i + 1
        });
        let (first, last) = (self.list[range.start].id, self.list[range.end - 1].id);
        self.drag = Some(BlockDrag { first, last, dx: 0.0, dy: 0.0, to: None });
        self.menu = None;
        self.block_menu = None;
        self.block_colors = false;
    }

    /// The blocks `g` drags: from its first to its last, with the blocks nested under that.
    fn dragged(&self, g: BlockDrag) -> Option<Range<usize>> {
        let (a, b) = (self.index(g.first)?, self.index(g.last)?);
        Some(a.min(b)..self.subtree(a.max(b)).end)
    }

    /// The "+" handle: opens the slash menu in block `id` when it's an empty paragraph, or in a
    /// new one below it.
    fn add_below(&mut self, id: u64) -> bool {
        let Some(i) = self.index(id) else { return false };
        let b = &self.list[i];
        let at = if b.kind == Kind::Paragraph && b.edit.text.is_empty() {
            self.focus(i, 0);
            i
        } else {
            let (at, indent) = (self.subtree(i).end, b.indent);
            self.insert(at, Kind::Paragraph, indent, "", Vec::new());
            at
        };
        self.selection = None;
        self.list[at].edit.insert("/");
        self.update_menu(at);
        true
    }

    /// Where dragging `g` by `g.dx, g.dy` would drop its blocks, given the rows' heights (a
    /// hidden one's 0): before which shown block, and how many levels deeper.
    fn drop_target(&self, g: BlockDrag, heights: &[f32]) -> Option<(usize, i8)> {
        let range = self.dragged(g)?;
        let h = |j: usize| heights.get(j).copied().unwrap_or(0.0);
        let top: f32 = (0..range.start).map(h).sum();
        let pointer = top + g.dy + h(range.start) * 0.5;
        let mut y = 0.0;
        let mut to = self.list.len();
        for j in 0..self.list.len() {
            let mid = y + h(j) * 0.5;
            y += h(j);
            if !range.contains(&j) && h(j) > 0.0 && mid > pointer {
                to = j;
                break;
            }
        }
        Some((to, (g.dx / INDENT).round().clamp(-8.0, 8.0) as i8))
    }

    /// While blocks are dragged: which, and how far the pointer moved them over the content.
    pub(super) fn ghost(&self) -> Option<(Range<usize>, [f32; 2])> {
        let g = self.drag.filter(BlockDrag::moved)?;
        Some((self.dragged(g)?, [g.dx, g.dy]))
    }

    /// Where the dragged blocks would land: the line's top below the first row (given the rows'
    /// heights), and the nesting level they'd take.
    pub(super) fn drop_line(&self, heights: &[f32]) -> Option<(f32, u8)> {
        let g = self.drag.filter(BlockDrag::moved)?;
        let (range, (to, by)) = (self.dragged(g)?, g.to?);
        if (range.start..=range.end).contains(&to) && by == 0 {
            return None;
        }
        let y = heights[..to.min(heights.len())].iter().sum();
        let above = (0..to).rev().find(|j| !range.contains(j));
        let limit = above.map_or(0, |j| (self.list[j].indent + 1).min(super::MAX_INDENT));
        let indent = (i16::from(self.list[range.start].indent) + i16::from(by)).clamp(0, i16::from(limit)) as u8;
        Some((y, indent))
    }

    /// The blocks `g` drags, if it moved them.
    pub(super) fn dragging(&self) -> Option<Range<usize>> {
        self.drag.filter(BlockDrag::moved).and_then(|g| self.dragged(g))
    }

    /// Drops `g`'s blocks where its drag points, and selects them; whether they moved.
    fn drop(&mut self, g: BlockDrag) -> bool {
        let (Some(range), Some((to, by))) = (self.dragged(g), g.to) else { return false };
        let moves = to < range.start || to > range.end;
        if !moves && by == 0 {
            return false;
        }
        let (first, last) = (self.list[range.start].id, self.list[range.end - 1].id);
        self.move_range(range, to, by);
        self.select(first, last);
        true
    }
}
