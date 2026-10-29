//! The toolbar over selected text, as Notion's: turn the block into another kind, bold,
//! italic, underline, strike, inline code, a link (⌘K) and a color. Colors also apply to whole
//! blocks, from a block's menu.

use rune::{Color, Cursor, Cx, El, Mark, Span, TextEdit, TextStyle, Tx, el, icon, rich, row, text};

use super::keys::field_key;
use super::menu::CHOICES;
use super::view::Wire;
use super::{Blocks, COLORS, Kind};
use crate::{Kit, Menu, MenuItem, TextInput};

/// The toolbar's height, and its gap above the text.
const HEIGHT: f32 = 34.0;
const GAP: f32 = 8.0;

/// The toolbar while one of its menus or its link field is open: the block and the text
/// selected in it, which those take the focus from.
#[derive(Clone, Debug, Default)]
pub(super) struct Toolbar {
    held: Option<Held>,
}

#[derive(Clone, Debug)]
struct Held {
    block: u64,
    selection: (usize, usize),
    menu: Option<Dropdown>,
    /// The link being written, while its field is open.
    link: Option<TextEdit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dropdown {
    TurnInto,
    Color,
}

impl Blocks {
    /// The block whose selected text the toolbar is for, while it shows.
    fn toolbar_block(&self) -> Option<usize> {
        if let Some(held) = &self.toolbar.held {
            return self.index(held.block);
        }
        if self.selection.is_some() || self.drag.is_some() || self.marquee.is_some() || self.menu.is_some() {
            return None;
        }
        let i = self.focused()?;
        let (a, z) = self.list[i].edit.range();
        (a != z && self.list[i].kind.is_rich()).then_some(i)
    }

    /// Holds the toolbar on block `i`'s selection while `menu` or the link field takes the
    /// focus.
    fn hold(&mut self, i: usize, menu: Option<Dropdown>, link: Option<TextEdit>) {
        let edit = &self.list[i].edit;
        let selection = (edit.anchor, edit.focus);
        let block = self.list[i].id;
        self.toolbar.held = Some(Held { block, selection, menu, link });
    }

    /// Lets go of the toolbar, focusing its block again with its text selected.
    fn release(&mut self) {
        let Some(held) = self.toolbar.held.take() else { return };
        let Some(i) = self.index(held.block) else { return };
        self.focus(i, 0);
        let (anchor, focus) = held.selection;
        self.list[i].edit.set_selection(anchor, focus);
    }

    /// The block whose text a format applies to: the toolbar's, or the focused one.
    fn text_target(&mut self) -> Option<usize> {
        let held = self.toolbar.held.as_ref().map(|h| (h.block, h.selection));
        if let Some((block, (anchor, focus))) = held {
            let i = self.index(block)?;
            self.list[i].edit.set_selection(anchor, focus);
            return Some(i);
        }
        self.focused()
    }

    /// Toggles `mark` on the selected text; whether the text changed.
    pub(super) fn format(&mut self, mark: Mark) -> bool {
        let Some(i) = self.text_target() else { return false };
        let edit = &mut self.list[i].edit;
        let before = edit.marks().to_vec();
        edit.toggle_mark(mark);
        edit.marks() != before
    }

    /// Colors the selected blocks' whole text, or else the selected text: `mark` is a
    /// [`Mark::Color`] or [`Mark::Highlight`], and `clear` takes that kind off instead.
    pub(super) fn paint(&mut self, mark: &Mark, clear: bool) -> bool {
        let whole = self.selected();
        let targets: Vec<usize> = match &whole {
            Some(range) => range.clone().collect(),
            None => self.text_target().into_iter().collect(),
        };
        let mut done = false;
        for i in targets {
            let b = &mut self.list[i];
            if !b.kind.is_rich() {
                continue;
            }
            let edit = &mut b.edit;
            let (anchor, focus) = (edit.anchor, edit.focus);
            if whole.is_some() {
                edit.set_selection(0, edit.text.len());
            }
            let before = edit.marks().to_vec();
            edit.remove_mark(mark);
            if !clear {
                edit.set_mark(mark.clone());
            }
            done |= edit.marks() != before;
            edit.set_selection(anchor, focus);
        }
        if !clear {
            self.last_color = mark.clone();
        }
        done
    }

    /// Opens the link field for the selected text (⌘K), with its link if it has one.
    pub(super) fn edit_link(&mut self) -> bool {
        let Some(i) = self.toolbar_block() else { return false };
        let url = self.list[i].edit.active_link().unwrap_or("").to_owned();
        let mut field = TextEdit::new(url);
        field.focused = true;
        field.set_selection(0, field.text.len());
        self.hold(i, None, Some(field));
        self.list[i].edit.focused = false;
        true
    }

    /// Links the selected text to what the link field holds (none when it's empty), and closes
    /// the field; whether the text changed.
    fn apply_link(&mut self) -> bool {
        let url = self.toolbar.held.as_ref().and_then(|h| h.link.as_ref()).map(|f| f.text.trim().to_owned());
        let Some(i) = self.text_target() else { return false };
        let edit = &mut self.list[i].edit;
        let before = edit.marks().to_vec();
        edit.set_link(url.as_deref());
        let done = edit.marks() != before;
        self.release();
        done
    }

    /// Turns the toolbar's block into `kind`, keeping its text selected.
    fn turn_selected_into(&mut self, kind: Kind) -> bool {
        let Some(i) = self.text_target() else { return false };
        let (anchor, focus) = (self.list[i].edit.anchor, self.list[i].edit.focus);
        self.toolbar.held = None;
        self.turn_into(i, kind);
        self.list[i].edit.set_selection(anchor, focus);
        true
    }
}

/// The toolbar, while text is selected in a block that takes formatting.
pub(super) fn toolbar<S: 'static>(k: &Kit, cx: &Cx<S>, doc: &Blocks, wire: &Wire<S>, layer: &mut Vec<El>) {
    let Some(i) = doc.toolbar_block() else { return };
    let b = &doc.list[i];
    let key = field_key(&wire.editor, b.id);
    let Some(field) = cx.layout(&key) else { return };
    // Above the selection's line when it's the first, else above the block.
    let (x, y) = match cx.caret(&key) {
        Some(c) if c[1] - field[1] < c[3] * 0.5 => ((c[0] - 160.0).max(field[0]), c[1]),
        Some(c) => ((c[0] - 160.0).max(field[0]), field[1]),
        None => (field[0], field[1]),
    };
    let held = doc.toolbar.held.as_ref();
    let mut bar = row()
        .key(format!("{}-toolbar", wire.editor))
        .absolute(x, (y - HEIGHT - GAP).max(4.0))
        .h(HEIGHT)
        .px(4.0)
        .gap(1.0)
        .center_y()
        .radius(8.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_weak)
        .preserve_focus_on_press(true);
    bar = crate::shadows(k, bar, &k.elevation.floating);
    if let Some(link) = held.and_then(|h| h.link.as_ref()) {
        layer.push(bar.child(link_field(k, link, wire)));
        return;
    }
    let edit = &b.edit;
    let open = held.and_then(|h| h.menu);
    // The menus anchor to their buttons, so they mount after the toolbar.
    let mut menus = Vec::new();
    let turn = turn_into_menu(k, wire, &b.kind, open == Some(Dropdown::TurnInto), &mut menus);
    bar = bar.child(turn).child(divider(k));
    let ink = k.c.text_base;
    let marks = [
        ("bold", rich("B", vec![Span::new(0, 1).weight(700.0)]), Mark::Bold),
        ("italic", rich("i", vec![Span::new(0, 1).italic()]), Mark::Italic),
        ("underline", rich("U", vec![Span::new(0, 1).underline(ink)]), Mark::Underline),
        ("strike", text("S"), Mark::Strike),
        ("code", text("</>").font(k.ty.mono), Mark::Code),
    ];
    for (name, glyph, mark) in marks {
        let active = edit.has_mark(&mark);
        let glyph = if mark == Mark::Strike { glyph.strike(0, 1, None) } else { glyph };
        let wire2 = wire.clone();
        let press = move |s: &mut S, tx: &mut Tx| wire2.edit(s, tx, |doc| doc.format(mark.clone()));
        bar = bar.child(button(k, name, glyph.color(ink), active).on_press_tx(press));
    }
    let wire2 = wire.clone();
    let link =
        button(k, "link", text("Link").color(ink), edit.active_link().is_some()).on_press_tx(move |s: &mut S, _| {
            (wire2.access)(s).edit_link();
        });
    bar = bar.child(divider(k)).child(link).child(divider(k));
    bar = bar.child(color_menu(k, wire, edit, open == Some(Dropdown::Color), &mut menus));
    layer.push(bar);
    layer.extend(menus);
}

fn divider(k: &Kit) -> El {
    el().w(1.0).h(18.0).mx(3.0).fill(k.c.border_weak)
}

fn button(k: &Kit, key: &str, content: El, active: bool) -> El {
    row()
        .key(key.to_owned())
        .h(26.0)
        .min_w(26.0)
        .px(6.0)
        .gap(4.0)
        .center()
        .radius(5.0)
        .fill(if active { k.c.layer_03 } else { Color::TRANSPARENT })
        .hover(k.c.hover)
        .hit()
        .cursor(Cursor::Pointer)
        .child(content.font(TextStyle { line_height: 18.0, ..k.ty.ui }))
}

/// The "Text ▾" button and its menu of kinds.
fn turn_into_menu<S: 'static>(k: &Kit, wire: &Wire<S>, kind: &Kind, open: bool, layer: &mut Vec<El>) -> El {
    let toggle = wire.clone();
    let trigger = button(k, "turn", text(kind.label()).color(k.c.text_base), false)
        .child(icon(k.glyph.chevron_down, 12.0).color(k.c.icon_muted))
        .on_press(move |s: &mut S| toggle_menu(&toggle, s, Dropdown::TurnInto));
    let close = wire.clone();
    let mut menu = Menu::new(format!("{}-turn-into", wire.editor))
        .trigger(trigger)
        .open(open)
        .min_width(220.0)
        .on_close(move |s: &mut S| (close.access)(s).release())
        .group_label("Turn into");
    for choice in CHOICES.iter().filter(|c| c.kind().has_text()) {
        let (target, wire) = (choice.kind(), wire.clone());
        let checked = target.same_kind(kind);
        menu = menu.item(MenuItem::new(target.label()).checked(checked).on_select_tx(move |s: &mut S, tx| {
            wire.edit(s, tx, |doc| doc.turn_selected_into(target.clone()));
        }));
    }
    menu.build(k, layer)
}

fn toggle_menu<S: 'static>(wire: &Wire<S>, s: &mut S, menu: Dropdown) {
    let doc = (wire.access)(s);
    if doc.toolbar.held.as_ref().is_some_and(|h| h.menu == Some(menu)) {
        doc.release();
    } else if let Some(i) = doc.toolbar_block() {
        doc.hold(i, Some(menu), None);
    }
}

/// The "A ▾" button and its menu of colors.
fn color_menu<S: 'static>(k: &Kit, wire: &Wire<S>, edit: &TextEdit, open: bool, layer: &mut Vec<El>) -> El {
    let marks = edit.active_marks();
    let current = |f: fn(&Mark) -> Option<u8>| marks.iter().find_map(f);
    let (color, background) = (current(|m| color_of(m, false)), current(|m| color_of(m, true)));
    let (colors, _) = crate::text_input::mark_colors(k);
    let ink = color.and_then(|n| colors.get(usize::from(n)).copied()).unwrap_or(k.c.text_base);
    let toggle = wire.clone();
    let trigger = button(k, "color", rich("A", vec![Span::new(0, 1).underline(ink)]).color(ink), false)
        .child(icon(k.glyph.chevron_down, 12.0).color(k.c.icon_muted))
        .on_press(move |s: &mut S| toggle_menu(&toggle, s, Dropdown::Color));
    let close = wire.clone();
    let mut menu = Menu::new(format!("{}-colors", wire.editor))
        .trigger(trigger)
        .open(open)
        .min_width(200.0)
        .on_close(move |s: &mut S| (close.access)(s).release());
    for item in color_items(k, wire, color, background) {
        menu = match item {
            ColorItem::Label(l) => menu.group_label(l),
            ColorItem::Item(i) => menu.item(*i),
        };
    }
    menu.build(k, layer)
}

/// The number of `mark` when it's a color (`background`: a highlight).
pub(super) fn color_of(mark: &Mark, background: bool) -> Option<u8> {
    match (mark, background) {
        (Mark::Color(n), false) | (Mark::Highlight(n), true) => Some(*n),
        _ => None,
    }
}

pub(super) enum ColorItem {
    Label(&'static str),
    Item(Box<MenuItem>),
}

/// Notion's color menu: the text colors, then the backgrounds, each with a default that takes
/// the color off; `color` and `background` are the ones the text has.
pub(super) fn color_items<S: 'static>(
    k: &Kit,
    wire: &Wire<S>,
    color: Option<u8>,
    background: Option<u8>,
) -> Vec<ColorItem> {
    let (colors, highlights) = crate::text_input::mark_colors(k);
    let mut items = Vec::new();
    for bg in [false, true] {
        items.push(ColorItem::Label(if bg { "Background" } else { "Color" }));
        let current = if bg { background } else { color };
        for n in std::iter::once(None).chain((0..COLORS.len() as u8).map(Some)) {
            let name = n.map_or("Default", |n| COLORS[usize::from(n)]);
            let label = if bg && n.is_some() { format!("{name} background") } else { name.to_owned() };
            let swatch = row().size(20.0, 20.0).center().radius(4.0).border(1.0, k.c.border_weak);
            let swatch = match n {
                Some(n) if bg => swatch.fill(highlights[usize::from(n)]),
                Some(n) => swatch.child(text("A").color(colors[usize::from(n)])),
                None => swatch.child(text("A").color(k.c.text_base)),
            };
            let mark = if bg { Mark::Highlight(n.unwrap_or(0)) } else { Mark::Color(n.unwrap_or(0)) };
            let wire = wire.clone();
            items.push(ColorItem::Item(Box::new(
                MenuItem::new(label).lead(swatch).checked(current == n).on_select_tx(move |s: &mut S, tx| {
                    wire.edit(s, tx, |doc| {
                        doc.block_menu = None;
                        let done = doc.paint(&mark, n.is_none());
                        doc.release();
                        done
                    });
                }),
            )));
        }
    }
    items
}

/// The link field: Return links the selected text, Escape closes it.
fn link_field<S: 'static>(k: &Kit, link: &TextEdit, wire: &Wire<S>) -> El {
    let (w1, w2, w3) = (wire.clone(), wire.clone(), wire.clone());
    TextInput::new(link)
        .placeholder("Paste a link")
        .width(Some(280.0))
        .on_edit(move |s: &mut S| link_edit((w1.access)(s)))
        .on_submit(move |s: &mut S, tx| w2.edit(s, tx, Blocks::apply_link))
        .on_key(move |s: &mut S, ev, _| {
            let escape = ev.key == rune::Key::Escape;
            if escape {
                (w3.access)(s).release();
            }
            escape
        })
        .build(k)
}

/// The link field's text (a scratch one once it's closed).
fn link_edit(doc: &mut Blocks) -> &mut TextEdit {
    match doc.toolbar.held.as_mut().and_then(|h| h.link.as_mut()) {
        Some(link) => link,
        None => &mut doc.scratch,
    }
}
