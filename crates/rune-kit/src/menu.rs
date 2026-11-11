//! Menu, context menu and their items (`@opencode/ui` navigation/menu, components/context-menu).
//! Compact (default) and standard appearances.
//!
//! Whether a menu is open is the app's (`open`, `on_close`); how it opens and closes is Rune's
//! modal stack (`runtime/input/modal_focus.rs`), which follows Desktop's menu machine
//! (`ui/src/navigation/menu/machine.ts`): focus returns to the trigger as the menu closes,
//! without a ring when the pointer opened it.
//!
//! An app drawing its own menus keeps this machinery and replaces the look: the surface
//! (`Menu::surface`), the rows' metrics and highlight (`Menu::row_height`, …), and a row's
//! content (`MenuItem::content`).

use std::rc::Rc;

use rune::{Color, El, Icon, PointerPress, TextStyle, Tx, col, el, icon, row, text};

use crate::overlay::{Placement, fit_scroll_bar, menu_anchor, scrim};
use crate::{Acts, Kit, State, shadows};

/// A menu at a secondary-press position. Store the point from `El::on_secondary_press`
/// in app state; clear it in `on_close` and item callbacks. Uses Menu's flipping anchor.
pub fn context_menu(key: impl Into<String>, trigger: El, point: Option<PointerPress>) -> Menu {
    let mut menu = Menu::new(key).trigger(trigger).open(point.is_some());
    menu.context = true;
    match point {
        Some(p) => menu.at(p.x, p.y),
        None => menu,
    }
}

pub enum MenuEntry {
    Item(Box<MenuItem>),
    Separator,
    Label(String),
    /// Anything else as a row of its own (a row of swatches, a field filtering a submenu).
    Custom(El),
}

impl MenuEntry {
    pub fn item(i: MenuItem) -> MenuEntry {
        MenuEntry::Item(Box::new(i))
    }
}

pub struct MenuItem {
    key: Option<String>,
    label: String,
    icon: Option<&'static Icon>,
    lead: Option<El>,
    shortcut: Option<String>,
    badge: Option<String>,
    description: Option<String>,
    danger: bool,
    /// Checkbox and radio items: whether the indicator shows.
    checked: Option<bool>,
    sub: Option<(Vec<MenuEntry>, bool)>,
    state: State,
    acts: Acts,
    content: Option<El>,
    with: Vec<Box<dyn FnOnce(El) -> El>>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> MenuItem {
        MenuItem {
            key: None,
            label: label.into(),
            icon: None,
            lead: None,
            shortcut: None,
            badge: None,
            description: None,
            danger: false,
            checked: None,
            sub: None,
            state: State::Rest,
            acts: Acts::default(),
            content: None,
            with: Vec::new(),
        }
    }
    /// The row's key (and its submenu's prefix), instead of `<menu>-item<n>`.
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }
    /// Draws the row's content, in place of its icon, label, description, shortcut, badge, check
    /// and chevron (and the faded look of a disabled row): those still say what the row is to
    /// assistive tech. The row keeps its height, padding, highlight and behaviour.
    pub fn content(mut self, e: El) -> Self {
        self.content = Some(e);
        self
    }
    /// Anything else on the row (a cursor, hover or key handlers), applied last, disabled or not.
    pub fn with(mut self, f: impl FnOnce(El) -> El + 'static) -> Self {
        self.with.push(Box::new(f));
        self
    }
    /// A 14 point icon before the label.
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    /// Anything before the label (an avatar, …).
    pub fn lead(mut self, e: El) -> Self {
        self.lead = Some(e);
        self
    }
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.badge = Some(b.into());
        self
    }
    /// A faint second line under the label, such as what a choice does. The item grows to fit it.
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    /// A destructive choice: the label and icon take the danger ink.
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// A checkbox or radio item, showing the check when `on`.
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }
    /// A submenu trigger; `open` shows its entries to the right.
    pub fn submenu(mut self, entries: Vec<MenuEntry>, open: bool) -> Self {
        self.sub = Some((entries, open));
        self
    }
    /// `Hover` paints the highlighted (keyboard-focused) row.
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        if d {
            self.state = State::Disabled;
        }
        self
    }
    pub fn on_select<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.acts.press(f);
        self
    }
    pub fn on_select_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.press_tx(f);
        self
    }

    /// The row's content as the appearance draws it: icon, label (and description), shortcut,
    /// badge and check.
    fn draw(&mut self, k: &Kit, standard: bool) -> Vec<El> {
        let c = &k.c;
        let disabled = self.state == State::Disabled;
        let (font, ink) = if standard {
            (k.ty.text_small_medium, if disabled { c.text_weak } else { c.text_strong })
        } else {
            (k.ty.ui_compact, c.text_base)
        };
        let danger = self.danger && !disabled;
        let ink = if danger { c.danger.fg } else { ink };
        // The item's label, shortcut, badge and check say what this row shows.
        let mut content = row().key("content").grow(1.0).shrink().gap(8.0).center_y().a11y_hidden();
        if let Some(l) = self.lead.take() {
            content = content.child(l);
        }
        if let Some(i) = self.icon {
            content = content.child(icon(i, 14.0).key("icon").color(if danger {
                c.danger.fg
            } else if standard {
                c.std_icon
            } else {
                c.icon_base
            }));
        }
        let label = text(self.label.clone()).key("label").font(font).color(ink).truncate();
        content = match &self.description {
            Some(d) => {
                let (font, ink) = if standard { (k.ty.text_xsmall, c.text_weak) } else { (k.ty.small, c.text_faint) };
                content.child(
                    col()
                        .key("text")
                        .grow(1.0)
                        .shrink()
                        .gap(2.0)
                        .child(label)
                        .child(text(d.clone()).key("description").font(font).color(ink).truncate()),
                )
            }
            None => content.child(label),
        };
        let mut children = vec![content];
        if let Some(s) = self.shortcut.clone() {
            let st = TextStyle { line_height: 11.0, ..k.ty.small_medium };
            children.push(text(s).key("shortcut").font(st).color(c.text_faint).a11y_hidden());
        }
        if let Some(b) = self.badge.clone() {
            children.push(
                row()
                    .a11y_hidden()
                    .key("badge")
                    .h(16.0)
                    .px(4.0)
                    .center()
                    .radius(2.0)
                    .fill(c.layer_02)
                    .border(0.5, c.border_base)
                    .child(text(b).font(TextStyle { line_height: 11.0, ..k.ty.small_medium }).color(c.text_muted)),
            );
        }
        if let Some(on) = self.checked {
            let mark = icon(k.glyph.check_small, 14.0)
                .color(if standard { c.std_icon } else { c.icon_base })
                .opacity(if on { 1.0 } else { 0.0 });
            children.push(el().key("indicator").size(16.0, 16.0).center().child(mark).a11y_hidden());
        }
        if self.sub.is_some() {
            // It points where the submenu opens: left, right to left (`menu.css`).
            children.push(
                icon(k.glyph.caret_right, 16.0).key("chevron").a11y_hidden().mirror_in_rtl().color(if standard {
                    c.std_icon
                } else {
                    c.text_faint
                }),
            );
        }
        children
    }

    fn build(mut self, k: &Kit, look: &Look, key: String, layer: &mut Vec<El>) -> El {
        let c = &k.c;
        let standard = look.standard;
        let key = self.key.take().unwrap_or(key);
        let disabled = self.state == State::Disabled;
        let highlighted = self.state == State::Hover || self.sub.as_ref().is_some_and(|s| s.1);
        let hover = look.highlight.unwrap_or(if standard { c.surface_raised_hover } else { c.hover });
        let drawn = self.content.is_some();
        let mut e = row()
            .key(key.clone())
            .role(if self.checked.is_some() { "menuitemcheckbox" } else { "menuitem" })
            .label(self.label.clone())
            .radius(look.row_radius.unwrap_or(if standard { k.radius.sm } else { 4.0 }))
            .gap(8.0)
            .center_y();
        if let Some(on) = self.checked {
            e = e.checked(on);
        }
        if let Some(s) = &self.shortcut {
            e = e.shortcut_hint(s.clone());
        }
        match (&self.description, &self.badge) {
            (Some(d), Some(b)) => e = e.description(format!("{d}, {b}")),
            (Some(d), None) | (None, Some(d)) => e = e.description(d.clone()),
            (None, None) => {}
        }
        if let Some((_, open)) = &self.sub {
            e = e.expanded(*open);
        }
        let (pl, pr) = look.row_padding.unwrap_or(if standard { (8.0, 8.0) } else { (12.0, 6.0) });
        let h = look.row_height.unwrap_or(28.0);
        e = match (standard && look.row_height.is_none(), self.description.is_some()) {
            // Drawn content sizes its row, at least a row high.
            _ if drawn => e.min_h(h).pl(pl).pr(pr),
            (true, _) => e.pl(pl).pr(pr).py(4.0),
            (false, false) => e.h(h).pl(pl).pr(pr),
            (false, true) => e.min_h(h).py(6.0).pl(pl).pr(pr),
        };
        if highlighted {
            e = e.fill(hover);
        } else if !disabled {
            // One highlight for pointer and keys, landing at once like NSMenu's.
            e = e.focus_highlight(hover);
        }
        e = match self.content.take() {
            Some(content) => e.child(content),
            None => e.children(self.draw(k, standard)),
        };
        if let Some((entries, true)) = self.sub {
            let sub = Menu {
                key: format!("{key}-sub"),
                entries,
                min_width: 160.0,
                look: Look { depth: look.depth + 1, ..look.clone() },
                ..Menu::new("")
            };
            let mut deeper = Vec::new();
            let surface = sub.frame(k, &mut deeper);
            let (x, y) = look.submenu_offset;
            let anchor = if y == 0.0 {
                key
            } else {
                // A point `y` down the row for the submenu to line up with.
                let anchor = format!("{key}-sub-anchor");
                e = e.child(el().key(anchor.clone()).absolute(0.0, y).w_full().h(1.0).inert().a11y_hidden());
                anchor
            };
            layer.push(menu_anchor(k, surface, &anchor, Placement::RightStart, x));
            layer.extend(deeper);
        }
        if disabled && !standard && !drawn {
            e = e.layer_opacity(0.5);
        }
        // A popup opens on its checked item (NSPopUpButton).
        let e = self
            .acts
            .apply(e.focusable(!disabled).focus_selected(self.checked == Some(true)).disabled(disabled), !disabled);
        self.with.into_iter().fold(e, |e, f| f(e))
    }
}

/// How a menu and its submenus look: the appearance, and what the app replaced of it.
#[derive(Clone, Default)]
struct Look {
    standard: bool,
    surface: Option<Rc<dyn Fn(El, usize) -> El>>,
    /// 0 for the menu, 1 for its submenu, …
    depth: usize,
    row_height: Option<f32>,
    row_padding: Option<(f32, f32)>,
    row_radius: Option<f32>,
    padding: Option<f32>,
    highlight: Option<Color>,
    submenu_offset: (f32, f32),
}

/// A dropdown menu: `Menu::new("file").trigger(button).item(..).open(s.open).build(&kit, &mut layer)`.
pub struct Menu {
    key: String,
    trigger: Option<El>,
    entries: Vec<MenuEntry>,
    open: bool,
    look: Look,
    placement: Placement,
    gutter: f32,
    min_width: f32,
    at: Option<(f32, f32)>,
    anchor: Option<String>,
    /// A context menu: its trigger is the area it opens over, not a button that opens it.
    context: bool,
    dismiss: Option<(El, Modal)>,
}

/// Makes an open surface modal: it takes focus, Escape and outside presses dismiss it, and
/// focus returns to the trigger when it closes.
pub(crate) type Modal = Box<dyn FnOnce(El) -> El>;

/// The outside-press scrim and the modal for a close request.
pub(crate) fn dismissal<S: 'static>(key: String, close: impl Fn(&mut S) + 'static) -> (El, Modal) {
    let close = Rc::new(close);
    let outside = close.clone();
    (scrim(key, move |s: &mut S| outside(s)), Box::new(move |e: El| e.modal(true, move |s: &mut S| close(s))))
}

/// Mounts a dismissal's scrim into `layer` and hands back its modal.
pub(crate) fn mount_dismiss(dismiss: Option<(El, Modal)>, layer: &mut Vec<El>) -> Option<Modal> {
    dismiss.map(|(scrim, modal)| {
        layer.push(scrim);
        modal
    })
}

impl Menu {
    pub fn new(key: impl Into<String>) -> Menu {
        Menu {
            key: key.into(),
            trigger: None,
            entries: Vec::new(),
            open: false,
            look: Look::default(),
            placement: Placement::BottomStart,
            gutter: 6.0,
            min_width: 160.0,
            at: None,
            anchor: None,
            context: false,
            dismiss: None,
        }
    }
    pub fn trigger(mut self, t: El) -> Self {
        self.trigger = Some(t);
        self
    }
    pub fn item(mut self, i: MenuItem) -> Self {
        self.entries.push(MenuEntry::Item(Box::new(i)));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(MenuEntry::Separator);
        self
    }
    pub fn group_label(mut self, l: impl Into<String>) -> Self {
        self.entries.push(MenuEntry::Label(l.into()));
        self
    }
    /// Adds entries built elsewhere (a list the app keeps).
    pub fn entries(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        self.entries.extend(entries);
        self
    }
    pub fn open(mut self, o: bool) -> Self {
        self.open = o;
        self
    }
    /// The older bordered look (`appearance="standard"`).
    pub fn standard(mut self) -> Self {
        self.look.standard = true;
        self.min_width = 128.0;
        self
    }
    pub fn placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }
    pub fn gutter(mut self, g: f32) -> Self {
        self.gutter = g;
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }
    /// A context menu: open at a window point (where the secondary press landed).
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.at = Some((x, y));
        self
    }
    /// Opens by the element of this key (with `placement` and `gutter`) rather than by its
    /// trigger: a menu built away from the button that opens it. Focus returns there.
    pub fn anchor(mut self, key: impl Into<String>) -> Self {
        self.anchor = Some(key.into());
        self
    }
    /// Presses outside and Escape close the menu. While open it holds keyboard focus.
    pub fn on_close<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.dismiss = Some(dismissal(format!("{}-scrim", self.key), f));
        self
    }
    /// Draws the surface of the menu and its submenus, in place of the appearance's fill, corners,
    /// border and shadow: `f(frame, depth)`, depth 0 for the menu, 1 for its submenu, …. The
    /// frame holds the rows; give it a fill, corners (clip them for a scrolling list), a width.
    pub fn surface(mut self, f: impl Fn(El, usize) -> El + 'static) -> Self {
        self.look.surface = Some(Rc::new(f));
        self
    }
    /// The rows' height (28 points; the standard look's rows fit their text).
    pub fn row_height(mut self, h: f32) -> Self {
        self.look.row_height = Some(h);
        self
    }
    /// The rows' padding before and after their content (12 and 6 points; standard 8 and 8).
    pub fn row_padding(mut self, before: f32, after: f32) -> Self {
        self.look.row_padding = Some((before, after));
        self
    }
    /// The rows' corner radius.
    pub fn row_radius(mut self, r: f32) -> Self {
        self.look.row_radius = Some(r);
        self
    }
    /// The padding between the surface and its rows (2 points; standard 4). Separators span it.
    pub fn padding(mut self, p: f32) -> Self {
        self.look.padding = Some(p);
        self
    }
    /// The fill of the row under the pointer or keys, and of a row whose submenu is open.
    pub fn highlight(mut self, c: Color) -> Self {
        self.look.highlight = Some(c);
        self
    }
    /// Where a submenu opens beside its row: `x` past the row's end (negative overlaps it), `y`
    /// below its top (`-padding` lines the submenu's first row up with it, as macOS does).
    pub fn submenu_offset(mut self, x: f32, y: f32) -> Self {
        self.look.submenu_offset = (x, y);
        self
    }

    fn frame(self, k: &Kit, layer: &mut Vec<El>) -> El {
        let c = &k.c;
        let look = self.look;
        let standard = look.standard;
        let frame =
            col().key(format!("{}-menu", self.key)).role("menu").focus_group().min_w(self.min_width).items_stretch();
        let frame = match &look.surface {
            Some(draw) => draw(frame, look.depth),
            None if standard => shadows(
                k,
                frame.radius(k.radius.md).fill(c.surface_raised).border(1.0, k.alpha(c.std_border, 0.5)),
                &k.elevation.md,
            ),
            None => shadows(k, frame.radius(6.0).fill(c.layer_01), &k.elevation.floating),
        };
        let padding = look.padding.unwrap_or(if standard { 4.0 } else { 2.0 });
        let mut list = col().key("rows").items_stretch().padding(padding);
        for (i, entry) in self.entries.into_iter().enumerate() {
            list = list.child(match entry {
                MenuEntry::Item(item) => item.build(k, &look, format!("{}-item{i}", self.key), layer),
                MenuEntry::Separator => {
                    let (m, color) = if standard { (4.0, c.border_weak) } else { (2.0, c.border_muted) };
                    el().key(format!("sep{i}")).h(1.0).mt(m).mb(m).ml(-padding).mr(-padding).fill(color).a11y_hidden()
                }
                MenuEntry::Label(l) => {
                    let e = row().key(format!("label{i}")).center_y();
                    if standard {
                        e.px(8.0).py(4.0).child(text(l).font(k.ty.text_xsmall_medium).color(c.text_weak))
                    } else {
                        e.h(28.0).px(12.0).child(text(l).font(k.ty.small_medium).color(c.text_faint))
                    }
                }
                MenuEntry::Custom(e) => e,
            });
        }
        frame.child(fit_scroll_bar(k, &self.key, list))
    }

    pub fn build(mut self, k: &Kit, layer: &mut Vec<El>) -> El {
        let anchor_key = format!("{}-trigger", self.key);
        // A dropdown's trigger reads as open or closed; a context menu's is just where it opened.
        let open = self.open;
        let pointer = self.context || self.at.is_some();
        let trigger = row()
            .key(anchor_key.clone())
            .children(self.trigger.take().map(|t| if pointer { t } else { t.expanded(open) }));
        if !self.open {
            return trigger;
        }
        let modal = mount_dismiss(self.dismiss.take(), layer);
        let returns = self.anchor.take().unwrap_or(anchor_key);
        let (placement, gutter, key) = match self.at {
            Some((x, y)) => {
                let point = format!("{}-point", self.key);
                layer.push(el().key(point.clone()).absolute(x, y).size(0.0, 0.0));
                (Placement::BottomStart, 0.0, point)
            }
            None => (self.placement, self.gutter, returns.clone()),
        };
        // Submenus anchor to items inside this surface, so they mount after it.
        let mut subs = Vec::new();
        let mut surface = self.frame(k, &mut subs).focus_return(returns);
        if let Some(modal) = modal {
            surface = modal(surface);
        }
        layer.push(menu_anchor(k, surface, &key, placement, gutter));
        layer.extend(subs);
        trigger
    }
}
