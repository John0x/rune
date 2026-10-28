//! [`BlockEditor`]: the document as rows of fields, with a gutter of handles, list markers,
//! dragging and the slash menu.

use std::rc::Rc;

use rune::{
    Align, Color, Cursor, Cx, Drag, El, Icon, Phase, RichStyle, Side, Span, TextStyle, Tx, col, edit_access, el, icon,
    row, text, text_input,
};

use super::keys::{Caret, Outcome, field_key};
use super::menu::matching;
use super::{Block, Blocks, Kind};
use crate::syntax::{SyntaxTheme, highlight};
use crate::{Kit, PopupItem, PopupList};

/// Width of the gutter left of the blocks, where a hovered block's handles show.
pub const GUTTER: f32 = 48.0;
/// Width of a nesting level.
const INDENT: f32 = 24.0;
/// Width of a list marker (bullet, number, checkbox).
const MARKER: f32 = 24.0;

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

/// A block being dragged by its handle: how far the pointer moved, and where it would land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BlockDrag {
    id: u64,
    dx: f32,
    dy: f32,
    /// The index it would land before, and how many levels deeper.
    to: Option<(usize, i8)>,
}

type Access<S> = Rc<dyn for<'b> Fn(&'b mut S) -> &'b mut Blocks>;
type ChangeFn<S> = Rc<dyn Fn(&mut S, &mut Tx)>;

/// What the handlers share: the document's accessor, the app's change hook, the editor's key.
struct Wire<S> {
    access: Access<S>,
    change: Option<ChangeFn<S>>,
    editor: Rc<str>,
}

impl<S> Clone for Wire<S> {
    fn clone(&self) -> Self {
        Wire { access: self.access.clone(), change: self.change.clone(), editor: self.editor.clone() }
    }
}

impl<S: 'static> Wire<S> {
    fn changed(&self, s: &mut S, tx: &mut Tx) {
        if let Some(change) = &self.change {
            change(s, tx);
        }
    }

    /// Runs `f` on the document; when it changed it, records the change and tells the app.
    fn edit(&self, s: &mut S, tx: &mut Tx, f: impl FnOnce(&mut Blocks) -> bool) {
        let doc = (self.access)(s);
        if f(doc) {
            doc.commit(None);
            self.changed(s, tx);
        }
    }
}

/// Type and colors, from the kit.
struct Look {
    body: TextStyle,
    headings: [TextStyle; 3],
    code: TextStyle,
    text: Color,
    muted: Color,
    faint: Color,
    fill: Color,
    rule: Color,
    selection: Color,
    /// Behind blocks selected whole.
    picked: Color,
    rich: RichStyle,
}

impl Look {
    fn new(k: &Kit) -> Look {
        let c = &k.c;
        Look {
            body: TextStyle::new(16.0, 400.0).lh(24.0),
            headings: [TextStyle::new(30.0, 650.0).lh(38.0), TextStyle::new(24.0, 620.0).lh(31.0), TextStyle::new(20.0, 600.0).lh(27.0)],
            code: TextStyle::new(14.0, 400.0).mono().lh(21.0),
            text: c.text_base,
            muted: c.text_muted,
            faint: c.text_faint,
            fill: c.layer_02,
            rule: c.border_base,
            selection: k.selection(),
            picked: k.c.text_accent.alpha(0.16),
            rich: RichStyle {
                link: c.text_accent,
                code_fg: None,
                code_bg: c.layer_03,
                code_block_bg: c.layer_02,
                quote_bar: c.border_strong,
                marker: Some(c.text_muted),
            },
        }
    }

    fn style(&self, kind: &Kind) -> TextStyle {
        match kind {
            Kind::Heading(level) => self.headings[(*level).clamp(1, 3) as usize - 1],
            Kind::Code { .. } => self.code,
            _ => self.body,
        }
    }

    /// Space above a block, more before a heading.
    fn space_above(&self, kind: &Kind) -> f32 {
        match kind {
            Kind::Heading(1) => 20.0,
            Kind::Heading(2) => 14.0,
            Kind::Heading(_) => 10.0,
            _ => 2.0,
        }
    }

    /// What an empty block shows.
    fn hint(&self, kind: &Kind) -> &'static str {
        match kind {
            Kind::Heading(1) => "Heading 1",
            Kind::Heading(2) => "Heading 2",
            Kind::Heading(_) => "Heading 3",
            Kind::Bullet | Kind::Numbered => "List",
            Kind::Todo { .. } => "To-do",
            Kind::Quote => "Empty quote",
            Kind::Code { .. } => "Code",
            _ => "Type '/' for commands",
        }
    }
}

/// A [`Blocks`] document's editor: `BlockEditor::new("note", &s.note).on_edit(|s: &mut App|
/// &mut s.note).build(&kit, cx, &mut layer)`. Without `on_edit` it only shows the document.
///
/// Each row leaves [`GUTTER`] points on its left for the handles a hovered block shows (add a
/// block, drag it); place the editor so that margin hangs outside the text column.
pub struct BlockEditor<'a, S> {
    key: String,
    doc: &'a Blocks,
    access: Option<Access<S>>,
    change: Option<ChangeFn<S>>,
    placeholder: String,
}

impl<'a, S: 'static> BlockEditor<'a, S> {
    pub fn new(key: impl Into<String>, doc: &'a Blocks) -> Self {
        BlockEditor { key: key.into(), doc, access: None, change: None, placeholder: String::new() }
    }

    /// What an empty document shows while it isn't focused.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Binds the editor to the document in app state.
    pub fn on_edit(mut self, f: impl for<'b> Fn(&'b mut S) -> &'b mut Blocks + 'static) -> Self {
        self.access = Some(Rc::new(f));
        self
    }

    /// Called after every change to the document (typing included): save it here.
    pub fn on_change(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    pub fn build(self, k: &Kit, cx: &Cx<S>, layer: &mut Vec<El>) -> El {
        let look = Look::new(k);
        let doc = self.doc;
        let wire = self.access.clone().map(|access| Wire { access, change: self.change.clone(), editor: self.key.as_str().into() });
        let heights: Rc<Vec<f32>> =
            Rc::new(doc.list.iter().map(|b| cx.layout_size(&row_key(&self.key, b.id)).map_or(0.0, |s| s[1])).collect());
        // Where the rows start on screen, while a field has focus (a drag selects across them).
        let top = if doc.is_focused() { cx.layout(&self.key).map_or(0.0, |r| r[1]) } else { 0.0 };
        let shifts = doc.drag.map(|d| doc.drag_shifts(d, &heights)).unwrap_or_default();
        let selected = doc.selected();
        let dragged = doc.drag.and_then(|d| doc.index(d.id)).map(|i| doc.subtree(i));
        let mut rows = col().key(self.key.clone()).w_full();
        for (i, b) in doc.list.iter().enumerate() {
            let row = Row {
                i,
                b,
                look: &look,
                selected: selected.as_ref().is_some_and(|r| r.contains(&i)),
                dragged: dragged.as_ref().is_some_and(|r| r.contains(&i)),
                shift: shifts.get(i).copied().unwrap_or([0.0, 0.0]),
                hint: if doc.is_empty() && !b.edit.focused && !self.placeholder.is_empty() {
                    Some(self.placeholder.as_str())
                } else if b.edit.focused || b.kind != Kind::Paragraph {
                    Some(look.hint(&b.kind))
                } else {
                    None
                },
            };
            rows = rows.child(row.build(k, cx, doc, &self.key, wire.as_ref(), &heights, top));
        }
        if let Some(wire) = &wire {
            rows = rows.child(tail(wire));
            menu(k, doc, wire, layer);
        }
        rows
    }
}

fn row_key(editor: &str, id: u64) -> String {
    format!("{editor}-b{id}")
}

/// The space below the blocks: a click there writes at the end.
fn tail<S: 'static>(wire: &Wire<S>) -> El {
    let wire = wire.clone();
    el().key(format!("{}-tail", wire.editor)).w_full().h(96.0).cursor(Cursor::Text).on_press_tx(move |s: &mut S, tx| {
        wire.edit(s, tx, |doc| {
            let last = doc.list.len() - 1;
            let b = &doc.list[last];
            if b.kind == Kind::Paragraph && b.edit.text.is_empty() {
                doc.focus(last, 0);
                return false;
            }
            doc.insert(last + 1, Kind::Paragraph, 0, "", Vec::new());
            true
        });
    })
}

/// One block's row, as the view builds it.
struct Row<'r> {
    i: usize,
    b: &'r Block,
    look: &'r Look,
    selected: bool,
    /// Whether it moves with a dragged block.
    dragged: bool,
    shift: [f32; 2],
    hint: Option<&'r str>,
}

impl Row<'_> {
    #[allow(clippy::too_many_arguments)]
    fn build<S: 'static>(
        self,
        k: &Kit,
        cx: &Cx<S>,
        doc: &Blocks,
        editor: &str,
        wire: Option<&Wire<S>>,
        heights: &Rc<Vec<f32>>,
        top: f32,
    ) -> El {
        let b = self.b;
        let style = self.look.style(&b.kind);
        let shown = doc.hovered == Some(b.id) || doc.drag.is_some_and(|d| d.id == b.id);
        let mut content = row()
            .key("content")
            .grow(1.0)
            .shrink()
            .items_start()
            .radius(4.0)
            .fill(if self.selected { self.look.picked } else { Color::TRANSPARENT })
            .child(el().w(f32::from(b.indent) * INDENT));
        if let Some(marker) = self.marker(k, doc, wire) {
            content = content.child(marker);
        }
        content = content.child(match &b.kind {
            Kind::Divider => {
                row().key("rule").grow(1.0).h(self.look.body.line_px()).center_y().child(el().w_full().h(1.0).fill(self.look.rule))
            }
            Kind::Code { .. } => {
                let field = self.field(k, cx, doc, editor, wire, (heights, top)).w_full();
                col().key("code").grow(1.0).shrink().radius(6.0).fill(self.look.fill).padding(14.0).child(field)
            }
            _ => self.field(k, cx, doc, editor, wire, (heights, top)).grow(1.0).shrink(),
        });
        let mut r = row()
            .key(row_key(editor, b.id))
            .w_full()
            .items_start()
            .pt(self.look.space_above(&b.kind))
            .pb(2.0)
            .offset(self.shift[0], self.shift[1])
            .opacity(if self.dragged { 0.55 } else { 1.0 })
            .child(gutter(style.line_px(), k.c.icon_muted, shown, b, wire, heights))
            .child(content);
        if let Some(wire) = wire {
            let (hover, id) = (wire.clone(), b.id);
            r = r.on_hover_change(move |s: &mut S, inside, _| {
                let doc = (hover.access)(s);
                if inside {
                    doc.hovered = Some(id);
                } else if doc.hovered == Some(id) {
                    doc.hovered = None;
                }
            });
            if b.kind == Kind::Divider {
                let wire = wire.clone();
                r = r.on_press(move |s: &mut S| (wire.access)(s).select(id, id));
            }
        }
        r
    }

    /// The bullet, number, checkbox or quote bar before a block's text.
    fn marker<S: 'static>(&self, k: &Kit, doc: &Blocks, wire: Option<&Wire<S>>) -> Option<El> {
        let b = self.b;
        let line = self.look.style(&b.kind).line_px();
        let glyph = |s: String| el().w(MARKER).h(line).center().child(text(s).font(self.look.body).color(self.look.text));
        Some(match &b.kind {
            Kind::Bullet => glyph(["•", "◦", "▪"][usize::from(b.indent) % 3].to_owned()),
            Kind::Numbered => glyph(format!("{}.", doc.number(self.i))),
            Kind::Todo { checked } => {
                let c = &k.c;
                let mut check = el()
                    .key("check")
                    .size(16.0, 16.0)
                    .center()
                    .radius(3.5)
                    .role("checkbox")
                    .label("Done")
                    .fill(if *checked { c.bg_accent } else { Color::TRANSPARENT })
                    .border(1.5, if *checked { c.bg_accent } else { self.look.muted })
                    .child(icon(k.glyph.check_small, 12.0).color(Color::WHITE).opacity(if *checked { 1.0 } else { 0.0 }));
                if let Some(wire) = wire {
                    let (wire, id) = (wire.clone(), b.id);
                    check = check.hit().cursor(Cursor::Pointer).on_press_tx(move |s: &mut S, tx| {
                        wire.edit(s, tx, |doc| doc.index(id).map(|i| doc.toggle_todo(i)).is_some());
                    });
                }
                el().w(MARKER).h(line).center().child(check)
            }
            Kind::Quote => el().w(3.0).self_stretch().mr(14.0).ml(2.0).radius(1.5).fill(self.look.text),
            _ => return None,
        })
    }

    /// The block's text field.
    fn field<S: 'static>(
        &self,
        k: &Kit,
        cx: &Cx<S>,
        doc: &Blocks,
        editor: &str,
        wire: Option<&Wire<S>>,
        (heights, top): (&Rc<Vec<f32>>, f32),
    ) -> El {
        let b = self.b;
        let look = self.look;
        let style = look.style(&b.kind);
        let key = field_key(editor, b.id);
        let checked = matches!(b.kind, Kind::Todo { checked: true });
        let hidden = doc.selection.is_some() && b.edit.focused;
        let mut f = text_input(&b.edit, style)
            .key(key.clone())
            .color(if checked { look.muted } else { look.text })
            .rich_style(look.rich)
            .caret(if hidden { Color::TRANSPARENT } else { look.text }, if hidden { Color::TRANSPARENT } else { look.selection });
        if let Some(hint) = self.hint {
            f = f.placeholder(hint, look.faint);
        }
        if let Kind::Code { language } = &b.kind {
            let lang = (!language.is_empty()).then_some(language.as_str());
            f = f.decorations(SyntaxTheme::from_kit(k).spans(&highlight(lang, &b.edit.text), 0));
        } else if checked {
            let mut done = Span::new(0, b.edit.text.len());
            done.color = Some(look.muted);
            f = f.decorations(vec![done]);
        }
        let Some(wire) = wire else { return f };
        let caret = b.edit.focused.then(|| caret(cx, &key, style.line_px())).flatten();
        let rows = Rows { heights: heights.clone(), top };
        bind(f, b.id, caret, rows, wire)
    }
}

/// Where the caret of the field keyed `key` is, from the last layout.
fn caret<S>(cx: &Cx<S>, key: &str, line: f32) -> Option<Caret> {
    let c = cx.caret(key)?;
    let r = cx.layout(key)?;
    Some(Caret {
        first_line: c[1] - r[1] < line * 0.5,
        last_line: (r[1] + r[3]) - (c[1] + c[3]) < line * 0.5,
        x: c[0],
        above: r[1] - 1.0,
        below: r[1] + r[3] + 1.0,
    })
}

/// The rows' heights and where the first starts on screen.
struct Rows {
    heights: Rc<Vec<f32>>,
    top: f32,
}

impl Rows {
    /// The row at window `y`.
    fn at(&self, y: f32) -> usize {
        let mut bottom = self.top;
        for (i, h) in self.heights.iter().enumerate() {
            bottom += h;
            if y < bottom {
                return i;
            }
        }
        self.heights.len().saturating_sub(1)
    }
}

/// A block field's handlers.
fn bind<S: 'static>(f: El, id: u64, caret: Option<Caret>, rows: Rows, wire: &Wire<S>) -> El {
    let (w1, w2, w3, w4, w5, w6) = (wire.clone(), wire.clone(), wire.clone(), wire.clone(), wire.clone(), wire.clone());
    f.on_edit(edit_access(move |s: &mut S| (w1.access)(s).edit_mut(id)))
        .on_input_key(move |s: &mut S, ev, tx| {
            let out = (w2.access)(s).key(id, ev, caret, &w2.editor, tx);
            if out == Outcome::Changed {
                w2.changed(s, tx);
            }
            out != Outcome::Ignored
        })
        .on_input_change(move |s: &mut S, tx| {
            (w3.access)(s).typed(id);
            w3.changed(s, tx);
        })
        .on_input_paste(move |s: &mut S, content, tx| {
            let done = (w4.access)(s).paste(id, content);
            if done {
                w4.changed(s, tx);
            }
            done
        })
        .on_input_press(move |s: &mut S, _| (w5.access)(s).pressed(id))
        .on_input_drag(move |s: &mut S, _, y, _| (w6.access)(s).drag_select(id, rows.at(y)))
}

/// The handles left of a block: add a block below, and drag (a click selects the block).
fn gutter<S: 'static>(line: f32, color: Color, shown: bool, b: &Block, wire: Option<&Wire<S>>, heights: &Rc<Vec<f32>>) -> El {
    let mut g = row().key("gutter").w(GUTTER).h(line).pr(4.0).justify_end().center_y().gap(2.0).opacity(if shown { 1.0 } else { 0.0 });
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
    let drag = {
        let (wire, heights) = (wire.clone(), heights.clone());
        button("drag", &HANDLE).hit().cursor(Cursor::Grab).on_drag(move |s: &mut S, d: &mut Drag, tx| {
            let doc = (wire.access)(s);
            match d.phase {
                Phase::Down => {
                    doc.drag = Some(BlockDrag { id, dx: 0.0, dy: 0.0, to: None });
                    doc.select(id, id);
                }
                Phase::Move => {
                    if let Some(mut g) = doc.drag {
                        (g.dx, g.dy) = (d.dx, d.dy);
                        g.to = doc.drop_target(g, &heights);
                        doc.drag = Some(g);
                    }
                }
                Phase::Up => {
                    let g = doc.drag.take();
                    if !d.cancelled()
                        && let Some(g) = g
                        && doc.drop(g)
                    {
                        doc.commit(None);
                        wire.changed(s, tx);
                    }
                }
            }
        })
    };
    g = g.child(add).child(drag);
    g
}

/// The slash menu, when open.
fn menu<S: 'static>(k: &Kit, doc: &Blocks, wire: &Wire<S>, layer: &mut Vec<El>) {
    let Some((m, query)) = doc.menu_query() else { return };
    let items: Vec<PopupItem> = matching(query)
        .into_iter()
        .map(|c| PopupItem::new(c.id, c.kind().label()).trailing(text(c.shortcut).font(k.ty.small).color(k.c.text_faint)))
        .collect();
    let (w1, w2, w3) = (wire.clone(), wire.clone(), wire.clone());
    PopupList::new(format!("{}-menu", wire.editor), field_key(&wire.editor, m.block))
        .anchored(field_key(&wire.editor, m.block), Side::Bottom, Align::Start, 6.0)
        .width(260.0)
        .max_height(340.0)
        .items(items)
        .active(Some(m.active))
        .empty_message("No matching blocks")
        .label("Turn into")
        .on_active(move |s: &mut S, i| {
            if let Some(m) = &mut (w1.access)(s).menu {
                m.active = i;
            }
        })
        .on_choose(move |s: &mut S, id, tx| w2.edit(s, tx, |doc| doc.menu_choose(Some(id))))
        .on_close(move |s: &mut S| (w3.access)(s).menu = None)
        .build(k, layer);
}

impl Blocks {
    /// A text selection dragged from block `id` over row `over`: blocks from one to the other
    /// once it leaves the block it started in.
    fn drag_select(&mut self, id: u64, over: usize) {
        let Some(i) = self.index(id) else { return };
        if over == i || over >= self.list.len() {
            self.selection = None;
        } else {
            self.selection = Some(super::Selection { anchor: id, head: self.list[over].id });
            self.menu = None;
        }
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

    /// Where dragging block `g.id` (and its children) by `g.dx, g.dy` would drop it, given the
    /// rows' heights: before which block, and how many levels deeper.
    fn drop_target(&self, g: BlockDrag, heights: &[f32]) -> Option<(usize, i8)> {
        let i = self.index(g.id)?;
        let range = self.subtree(i);
        let h = |j: usize| heights.get(j).copied().unwrap_or(0.0);
        let top: f32 = (0..range.start).map(h).sum();
        let center = top + g.dy + range.clone().map(h).sum::<f32>() * 0.5;
        let mut y = 0.0;
        let mut to = self.list.len();
        for j in 0..self.list.len() {
            let mid = y + h(j) * 0.5;
            y += h(j);
            if !range.contains(&j) && mid > center {
                to = j;
                break;
            }
        }
        Some((to, (g.dx / INDENT).round().clamp(-8.0, 8.0) as i8))
    }

    /// How far each row is drawn from its place while `g` drags: the dragged ones with the
    /// pointer, the ones between their place and the target a step aside.
    fn drag_shifts(&self, g: BlockDrag, heights: &[f32]) -> Vec<[f32; 2]> {
        let mut shifts = vec![[0.0, 0.0]; self.list.len()];
        let Some(i) = self.index(g.id) else { return shifts };
        let range = self.subtree(i);
        let moved: f32 = range.clone().map(|j| heights.get(j).copied().unwrap_or(0.0)).sum();
        let (to, by) = g.to.unwrap_or((range.end, 0));
        for (j, shift) in shifts.iter_mut().enumerate() {
            if range.contains(&j) {
                *shift = [f32::from(by) * INDENT, g.dy];
            } else if j >= range.end && j < to {
                shift[1] = -moved;
            } else if j >= to && j < range.start {
                shift[1] = moved;
            }
        }
        shifts
    }

    /// Drops a dragged block where its drag points; whether it moved.
    fn drop(&mut self, g: BlockDrag) -> bool {
        let (Some(i), Some((to, by))) = (self.index(g.id), g.to) else { return false };
        let range = self.subtree(i);
        let moves = to < range.start || to > range.end;
        if !moves && by == 0 {
            return false;
        }
        let first = self.list[range.start].id;
        let last = self.list[range.end - 1].id;
        self.move_range(range, to, by);
        self.select(first, last);
        true
    }
}
