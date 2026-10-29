//! [`BlockEditor`]: the document as rows of fields, with a gutter of handles, list markers,
//! dragging, the slash menu and the formatting toolbar.

use std::rc::Rc;

use rune::{
    Align, Color, Cursor, Cx, El, RichStyle, Side, Span, TextStyle, Tx, col, edit_access, el, icon, row, text,
    text_input,
};

use super::handle::{self, GUTTER, INDENT};
use super::keys::{Caret, Outcome, field_key};
use super::marquee::{self, Rows};
use super::menu::matching;
use super::{Block, Blocks, Kind, toolbar};
use crate::syntax::{SyntaxTheme, highlight};
use crate::{Kit, PopupItem, PopupList};

/// Width of a list marker (bullet, number, checkbox).
const MARKER: f32 = 24.0;

type Access<S> = Rc<dyn for<'b> Fn(&'b mut S) -> &'b mut Blocks>;
type ChangeFn<S> = Rc<dyn Fn(&mut S, &mut Tx)>;
/// The app's styling of a block's text (see [`BlockEditor::decorate`]).
type Decorate<'a> = Box<dyn Fn(&Block) -> Vec<Span> + 'a>;
/// The app's element at the end of a block's row (see [`BlockEditor::aside`]).
type Aside<'a> = Box<dyn Fn(&Block, bool) -> Option<El> + 'a>;

/// Width of the column [`BlockEditor::aside`] takes at the end of each row.
const ASIDE: f32 = 28.0;

/// What the handlers share: the document's accessor, the app's change hook, the editor's key.
pub(super) struct Wire<S> {
    pub(super) access: Access<S>,
    pub(super) change: Option<ChangeFn<S>>,
    pub(super) editor: Rc<str>,
}

impl<S> Clone for Wire<S> {
    fn clone(&self) -> Self {
        Wire { access: self.access.clone(), change: self.change.clone(), editor: self.editor.clone() }
    }
}

impl<S: 'static> Wire<S> {
    pub(super) fn changed(&self, s: &mut S, tx: &mut Tx) {
        if let Some(change) = &self.change {
            change(s, tx);
        }
    }

    /// Runs `f` on the document; when it changed it, records the change and tells the app.
    pub(super) fn edit(&self, s: &mut S, tx: &mut Tx, f: impl FnOnce(&mut Blocks) -> bool) {
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
    /// Where dragged blocks would land.
    drop: Color,
    rich: RichStyle,
}

impl Look {
    fn new(k: &Kit) -> Look {
        let c = &k.c;
        Look {
            body: TextStyle::new(16.0, 400.0).lh(24.0),
            headings: [
                TextStyle::new(30.0, 650.0).lh(38.0),
                TextStyle::new(24.0, 620.0).lh(31.0),
                TextStyle::new(20.0, 600.0).lh(27.0),
            ],
            code: TextStyle::new(14.0, 400.0).mono().lh(21.0),
            text: c.text_base,
            muted: c.text_muted,
            faint: c.text_faint,
            fill: c.layer_02,
            rule: c.border_base,
            selection: k.selection(),
            picked: k.c.text_accent.alpha(0.16),
            drop: k.c.text_accent.alpha(0.55),
            rich: RichStyle {
                link: c.text_accent,
                code_fg: None,
                code_bg: c.layer_03,
                code_block_bg: c.layer_02,
                quote_bar: c.border_strong,
                marker: Some(c.text_muted),
                colors: crate::text_input::mark_colors(k).0,
                highlights: crate::text_input::mark_colors(k).1,
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
            Kind::Toggle { .. } => "Toggle",
            Kind::Quote => "Empty quote",
            Kind::Code { .. } => "Code",
            _ => "Type '/' for commands",
        }
    }
}

/// A [`Blocks`] document's editor: `BlockEditor::new("note", &s.note).on_edit(|s: &mut App|
/// &mut s.note).build(&kit, cx, &mut layer)`. Without `on_edit` it only shows the document.
///
/// Each row leaves [`GUTTER`](super::GUTTER) points on its left for the handles a hovered block shows (add a
/// block, drag it); place the editor so that margin hangs outside the text column.
pub struct BlockEditor<'a, S> {
    key: String,
    doc: &'a Blocks,
    access: Option<Access<S>>,
    change: Option<ChangeFn<S>>,
    placeholder: String,
    decorate: Option<Decorate<'a>>,
    aside: Option<Aside<'a>>,
}

impl<'a, S: 'static> BlockEditor<'a, S> {
    pub fn new(key: impl Into<String>, doc: &'a Blocks) -> Self {
        BlockEditor {
            key: key.into(),
            doc,
            access: None,
            change: None,
            placeholder: String::new(),
            decorate: None,
            aside: None,
        }
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

    /// Styles ranges of each block's text (byte offsets into [`Block::text`]): a color for
    /// what someone else wrote, a highlight for a match. Code keeps its syntax colors.
    pub fn decorate(mut self, f: impl Fn(&Block) -> Vec<Span> + 'a) -> Self {
        self.decorate = Some(Box::new(f));
        self
    }

    /// An element at the end of each block's row, in a column of its own: a mark that shows
    /// where the block comes from, say. `f` gets the block and whether the pointer is on its
    /// row, and returns `None` to leave the column empty.
    pub fn aside(mut self, f: impl Fn(&Block, bool) -> Option<El> + 'a) -> Self {
        self.aside = Some(Box::new(f));
        self
    }

    pub fn build(self, k: &Kit, cx: &Cx<S>, layer: &mut Vec<El>) -> El {
        let look = Look::new(k);
        let doc = self.doc;
        let wire = self.access.clone().map(|access| Wire {
            access,
            change: self.change.clone(),
            editor: self.key.as_str().into(),
        });
        let geometry = Rows::read(cx, &self.key, doc);
        let heights = geometry.heights.clone();
        // Keyed after the editor, so the app's own keys don't stand in for it.
        let rows_key = format!("{}-rows", self.key);
        let origin = cx.layout_with(&rows_key, |r| [r[0], r[1], r[2]]);
        let selected = doc.selected();
        let dragged = doc.dragging();
        let hidden = doc.hidden();
        let mut rows = col().key(rows_key).w_full();
        for (i, b) in doc.list.iter().enumerate() {
            if hidden[i] {
                continue;
            }
            let row = self.row(&look, doc, i, b);
            let row = Row {
                selected: selected.as_ref().is_some_and(|r| r.contains(&i)),
                dragged: dragged.as_ref().is_some_and(|r| r.contains(&i)),
                ..row
            };
            rows = rows.child(row.build(k, cx, doc, &self.key, wire.as_ref(), &geometry));
        }
        if let Some(wire) = &wire {
            rows = rows
                .child(el().key(format!("{}-tail", self.key)).w_full().h(96.0).cursor(Cursor::Text))
                .drag_autoscroll()
                .on_drag(marquee::handler(wire, geometry.clone(), origin.map(|o| [o[0], o[1]])));
            menu(k, doc, wire, layer);
            handle::block_menu(k, doc, wire, layer);
            toolbar::toolbar(k, cx, doc, wire, layer);
        }
        if let Some(rectangle) = marquee::rectangle(k, doc, origin.map(|o| [o[0], o[1]])) {
            rows = rows.child(rectangle);
        }
        let width = origin.map_or(0.0, |o| o[2]);
        if let Some((y, indent)) = doc.drop_line(&heights) {
            let x = GUTTER + f32::from(indent) * INDENT;
            let line = el().key("drop").absolute(x, y - 2.0).size((width - x).max(0.0), 4.0).radius(2.0);
            rows = rows.child(line.fill(look.drop).raise());
        }
        if let Some(ghost) = self.ghost(k, cx, &look, &geometry, width) {
            rows = rows.child(ghost);
        }
        rows
    }

    /// Block `i`'s row, before the view marks it selected or dragged.
    fn row<'r>(&'r self, look: &'r Look, doc: &Blocks, i: usize, b: &'r Block) -> Row<'r> {
        Row {
            i,
            b,
            look,
            selected: false,
            dragged: false,
            hint: if doc.is_empty() && !b.edit.focused && !self.placeholder.is_empty() {
                Some(self.placeholder.as_str())
            } else if b.edit.focused || b.kind != Kind::Paragraph {
                Some(look.hint(&b.kind))
            } else {
                None
            },
            spans: self.decorate.as_ref().map(|f| f(b)).unwrap_or_default(),
            aside: self.aside.as_ref().map(|f| f(b, doc.hovered == Some(b.id))),
        }
    }

    /// A faded copy of the dragged blocks under the pointer, as Notion's.
    fn ghost(&self, k: &Kit, cx: &Cx<S>, look: &Look, rows: &Rows, width: f32) -> Option<El> {
        let doc = self.doc;
        let (range, [dx, dy]) = doc.ghost()?;
        let top: f32 = rows.heights[..range.start.min(rows.heights.len())].iter().sum();
        let hidden = doc.hidden();
        let editor = format!("{}-ghost", self.key);
        let mut ghost = col().key(editor.clone()).absolute(dx, top + dy).w(width).opacity(0.5).raise();
        for i in range.filter(|&i| !hidden[i]) {
            let mut b = doc.list[i].clone();
            b.edit.focused = false;
            let row = self.row(look, doc, i, &b);
            ghost = ghost.child(row.build(k, cx, doc, &editor, None::<&Wire<S>>, rows));
        }
        Some(ghost)
    }
}

pub(super) fn row_key(editor: &str, id: u64) -> String {
    format!("{editor}-b{id}")
}

/// One block's row, as the view builds it.
struct Row<'r> {
    i: usize,
    b: &'r Block,
    look: &'r Look,
    selected: bool,
    /// Whether it's being dragged (a copy follows the pointer).
    dragged: bool,
    hint: Option<&'r str>,
    /// The app's decorations of its text.
    spans: Vec<Span>,
    /// The app's element at its end, when the editor has a column for one.
    aside: Option<Option<El>>,
}

impl Row<'_> {
    fn build<S: 'static>(
        self,
        k: &Kit,
        cx: &Cx<S>,
        doc: &Blocks,
        editor: &str,
        wire: Option<&Wire<S>>,
        rows: &Rows,
    ) -> El {
        let b = self.b;
        let style = self.look.style(&b.kind);
        let shown = doc.hovered == Some(b.id) || self.dragged;
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
            Kind::Divider => row()
                .key("rule")
                .grow(1.0)
                .h(self.look.body.line_px())
                .center_y()
                .child(el().w_full().h(1.0).fill(self.look.rule)),
            Kind::Code { .. } => {
                let field = self.field(k, cx, doc, editor, wire, rows).w_full();
                col().key("code").grow(1.0).shrink().radius(6.0).fill(self.look.fill).padding(14.0).child(field)
            }
            _ => self.field(k, cx, doc, editor, wire, rows).grow(1.0).shrink(),
        });
        // Where its menu opens: under ⋮⋮, read while the handles show.
        let anchor = shown
            .then(|| cx.layout_with(&format!("{}/gutter/drag", row_key(editor, b.id)), |r| [r[0], r[1] + r[3]]))
            .flatten();
        let mut r = row()
            .key(row_key(editor, b.id))
            .w_full()
            .items_start()
            .pt(self.look.space_above(&b.kind))
            .pb(2.0)
            .opacity(if self.dragged { 0.4 } else { 1.0 })
            .child(handle::gutter(style.line_px(), k.c.icon_muted, shown, b, wire, &rows.heights, anchor))
            .child(content);
        if let Some(aside) = self.aside {
            r = r.child(el().key("aside").w(ASIDE).h(style.line_px()).center().children(aside));
        }
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
        let glyph =
            |s: String| el().w(MARKER).h(line).center().child(text(s).font(self.look.body).color(self.look.text));
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
                    .child(icon(k.glyph.check_small, 12.0).color(Color::WHITE).opacity(if *checked {
                        1.0
                    } else {
                        0.0
                    }));
                if let Some(wire) = wire {
                    let (wire, id) = (wire.clone(), b.id);
                    check = check.hit().cursor(Cursor::Pointer).on_press_tx(move |s: &mut S, tx| {
                        wire.edit(s, tx, |doc| doc.index(id).map(|i| doc.toggle_todo(i)).is_some());
                    });
                }
                el().w(MARKER).h(line).center().child(check)
            }
            Kind::Quote => el().w(3.0).self_stretch().mr(14.0).ml(2.0).radius(1.5).fill(self.look.text),
            Kind::Toggle { open } => {
                let glyph = if *open { k.glyph.triangle_down } else { k.glyph.triangle_right };
                let mut arrow = el()
                    .key("toggle")
                    .size(20.0, 20.0)
                    .center()
                    .radius(4.0)
                    .role("button")
                    .label(if *open { "Close" } else { "Open" })
                    .child(icon(glyph, 12.0).color(self.look.text));
                if let Some(wire) = wire {
                    let (wire, id) = (wire.clone(), b.id);
                    arrow = arrow.hit().cursor(Cursor::Pointer).hover(k.c.hover).on_press_tx(move |s: &mut S, tx| {
                        wire.edit(s, tx, |doc| doc.index(id).is_some_and(|i| doc.toggle_open(i)));
                    });
                }
                el().w(MARKER).h(line).center().child(arrow)
            }
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
        rows: &Rows,
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
            .caret(
                if hidden { Color::TRANSPARENT } else { look.text },
                if hidden { Color::TRANSPARENT } else { look.selection },
            );
        if let Some(hint) = self.hint {
            f = f.placeholder(hint, look.faint);
        }
        if let Kind::Code { language } = &b.kind {
            let lang = (!language.is_empty()).then_some(language.as_str());
            f = f.decorations(SyntaxTheme::from_kit(k).spans(&highlight(lang, &b.edit.text), 0));
        } else if checked || !self.spans.is_empty() {
            let mut spans = self.spans.clone();
            if checked {
                let mut done = Span::new(0, b.edit.text.len());
                done.color = Some(look.muted);
                spans.push(done);
            }
            f = f.decorations(spans);
        }
        let Some(wire) = wire else { return f };
        let caret = b.edit.focused.then(|| caret(cx, &key, style.line_px())).flatten();
        let rows = rows.clone();
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
        .on_input_press(move |s: &mut S, tx| (w5.access)(s).pressed(id, tx.mods().shift))
        .on_input_drag(move |s: &mut S, _, y, _| {
            if let Some(over) = rows.at(y) {
                (w6.access)(s).drag_select(id, over);
            }
        })
}

/// The slash menu, when open.
fn menu<S: 'static>(k: &Kit, doc: &Blocks, wire: &Wire<S>, layer: &mut Vec<El>) {
    let Some((m, query)) = doc.menu_query() else { return };
    let items: Vec<PopupItem> = matching(query)
        .into_iter()
        .map(|c| {
            PopupItem::new(c.id, c.kind().label()).trailing(text(c.shortcut).font(k.ty.small).color(k.c.text_faint))
        })
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
}
