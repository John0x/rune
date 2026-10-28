//! Keys in a block's field, before the field sees them: structure (Return, Backspace, Tab),
//! moving between blocks, Markdown shortcuts, the slash menu, and whole-block selection.

use rune::{ClipboardContent, Key, KeyEvent, Tx};

use super::{Blocks, Kind, Selection, inline, markdown, model};

/// Where the focused block's caret is, as the view read it from the last layout (window
/// points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Caret {
    pub(super) first_line: bool,
    pub(super) last_line: bool,
    pub(super) x: f32,
    /// Just above and just below the field.
    pub(super) above: f32,
    pub(super) below: f32,
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    /// Not ours: the field handles it.
    Ignored,
    /// Taken, the document unchanged.
    Handled,
    /// Taken, and the document changed.
    Changed,
}

fn changed(done: bool) -> Outcome {
    if done { Outcome::Changed } else { Outcome::Handled }
}

/// The key of block `id`'s field in the editor keyed `editor`.
pub(super) fn field_key(editor: &str, id: u64) -> String {
    format!("{editor}-f{id}")
}

impl Blocks {
    /// A key in block `id`'s field.
    pub(super) fn key(&mut self, id: u64, ev: &KeyEvent, caret: Option<Caret>, editor: &str, tx: &mut Tx) -> Outcome {
        let Some(i) = self.index(id) else { return Outcome::Ignored };
        if self.selection.is_some() {
            return self.selection_key(ev, tx);
        }
        if self.menu.is_some_and(|m| m.block == id)
            && let Some(out) = self.menu_key(ev)
        {
            return out;
        }
        let out = self.edit_key(i, ev, caret, editor, tx);
        if out == Outcome::Changed {
            self.commit(None);
        }
        out
    }

    fn menu_key(&mut self, ev: &KeyEvent) -> Option<Outcome> {
        let m = ev.mods;
        if m.cmd || m.alt || m.ctrl {
            return None;
        }
        Some(match ev.key {
            Key::Up => {
                self.menu_step(-1);
                Outcome::Handled
            }
            Key::Down => {
                self.menu_step(1);
                Outcome::Handled
            }
            Key::Enter | Key::Tab => {
                let done = self.menu_choose(None);
                if done {
                    self.commit(None);
                }
                changed(done)
            }
            Key::Escape => {
                self.menu = None;
                Outcome::Handled
            }
            _ => return None,
        })
    }

    fn edit_key(&mut self, i: usize, ev: &KeyEvent, caret: Option<Caret>, editor: &str, tx: &mut Tx) -> Outcome {
        let m = ev.mods;
        let plain = !m.cmd && !m.alt && !m.ctrl;
        let b = &self.list[i];
        let (a, z) = b.edit.range();
        let len = b.edit.text.len();
        let code = matches!(b.kind, Kind::Code { .. });
        if ev.is_command('z') {
            return changed(if m.shift { self.redo() } else { self.undo() });
        }
        if ev.is_command('a') && a == 0 && z == len {
            self.select(self.list[0].id, self.list[self.list.len() - 1].id);
            return Outcome::Handled;
        }
        match &ev.key {
            Key::Up | Key::Down if m.cmd && m.shift => {
                let range = self.subtree(i);
                changed(self.move_by(range, ev.key == Key::Down))
            }
            Key::Enter if m.cmd && matches!(b.kind, Kind::Todo { .. }) => {
                self.toggle_todo(i);
                Outcome::Changed
            }
            Key::Enter if plain && !m.shift && code => {
                self.list[i].edit.insert("\n");
                Outcome::Changed
            }
            Key::Enter if plain && !m.shift => {
                self.enter(i);
                Outcome::Changed
            }
            Key::Backspace if plain && a == 0 && z == 0 => changed(self.backspace_at_start(i)),
            Key::Delete if plain && a == len && z == len => changed(self.delete_at_end(i)),
            Key::Tab if plain && code && !m.shift => {
                self.list[i].edit.insert("  ");
                Outcome::Changed
            }
            Key::Tab if plain => changed(self.nest(i, if m.shift { -1 } else { 1 })),
            Key::Up | Key::Down if plain => self.arrow(i, ev.key == Key::Down, m.shift, caret, editor, tx),
            Key::Left if plain && !m.shift && a == 0 && z == 0 => self.step(i, false),
            Key::Right if plain && !m.shift && a == len && z == len => self.step(i, true),
            Key::Escape if plain => {
                self.select(self.list[i].id, self.list[i].id);
                Outcome::Handled
            }
            Key::Text(t) if plain && !code && self.shortcut(i, t) => Outcome::Changed,
            _ => Outcome::Ignored,
        }
    }

    /// Up or Down on the field's first or last line moves to the block above or below, at the
    /// caret's column; with Shift at the very start or end, it selects blocks.
    fn arrow(&mut self, i: usize, down: bool, shift: bool, caret: Option<Caret>, editor: &str, tx: &mut Tx) -> Outcome {
        let b = &self.list[i];
        let (a, z) = b.edit.range();
        let text = &b.edit.text;
        let edge = match caret {
            Some(c) => if down { c.last_line } else { c.first_line },
            None => if down { !text[z..].contains('\n') } else { !text[..a].contains('\n') },
        };
        if !edge {
            return Outcome::Ignored;
        }
        if shift {
            let at_end = if down { b.edit.focus == text.len() } else { b.edit.focus == 0 };
            if !at_end {
                return Outcome::Ignored;
            }
            let next = if down { (i + 1 < self.list.len()).then_some(i + 1) } else { i.checked_sub(1) };
            let id = b.id;
            self.select(id, next.map_or(id, |n| self.list[n].id));
            return Outcome::Handled;
        }
        let Some(j) = self.text_block(i, down) else { return Outcome::Ignored };
        match caret {
            Some(c) => tx.place_caret(field_key(editor, self.list[j].id), c.x, if down { c.below } else { c.above }),
            None => {
                let end = if down { 0 } else { self.list[j].edit.text.len() };
                self.focus(j, end);
            }
        }
        Outcome::Handled
    }

    /// Left at a block's start or Right at its end moves into the block beside it.
    fn step(&mut self, i: usize, forward: bool) -> Outcome {
        let Some(j) = self.text_block(i, forward) else { return Outcome::Ignored };
        let caret = if forward { 0 } else { self.list[j].edit.text.len() };
        self.focus(j, caret);
        Outcome::Handled
    }

    /// The nearest block holding text after (or before) block `i`.
    pub(super) fn text_block(&self, i: usize, forward: bool) -> Option<usize> {
        if forward {
            (i + 1..self.list.len()).find(|&j| self.list[j].kind.has_text())
        } else {
            (0..i).rev().find(|&j| self.list[j].kind.has_text())
        }
    }

    /// A Markdown shortcut that typing `t` completes in block `i`: a kind at its start, or
    /// formatting around text.
    fn shortcut(&mut self, i: usize, t: &str) -> bool {
        let mut chars = t.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else { return false };
        let b = &mut self.list[i];
        let (a, z) = b.edit.range();
        if a != z {
            return false;
        }
        let before = &b.edit.text[..a];
        let kind = match c {
            ' ' => inline::block_prefix(before).filter(|k| *k != b.kind),
            '`' if before == "``" => Some(Kind::Code { language: String::new() }),
            '-' if before == "--" && a == b.edit.text.len() => Some(Kind::Divider),
            _ => None,
        };
        if let Some(kind) = kind {
            b.edit.set_selection(0, a);
            b.edit.insert("");
            self.turn_into(i, kind);
            return true;
        }
        b.kind.is_rich() && inline::close_format(&mut b.edit, c)
    }

    /// Selects blocks from `anchor` to `head`, keeping a field focused to take the keys.
    pub(super) fn select(&mut self, anchor: u64, head: u64) {
        self.selection = Some(Selection { anchor, head });
        self.menu = None;
        if let Some(h) = self.index(head) {
            self.keep_keys(h);
        }
    }

    /// Focuses a field near block `i` (a divider has none), so keys keep coming while blocks
    /// are selected.
    fn keep_keys(&mut self, i: usize) {
        let j = if self.list[i].kind.has_text() { Some(i) } else { self.text_block(i, false).or_else(|| self.text_block(i, true)) };
        if let Some(j) = j
            && !self.list[j].edit.focused
        {
            let end = self.list[j].edit.text.len();
            self.focus(j, end);
        }
    }

    /// A key while blocks are selected.
    fn selection_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> Outcome {
        let (Some(range), Some(sel)) = (self.selected(), self.selection) else {
            self.selection = None;
            return Outcome::Ignored;
        };
        let m = ev.mods;
        let plain = !m.cmd && !m.alt && !m.ctrl;
        let head = self.index(sel.head).unwrap_or(range.start);
        let out = match &ev.key {
            Key::Escape | Key::Enter if plain => {
                self.selection = None;
                if let Some(j) = self.list[head].kind.has_text().then_some(head).or_else(|| self.text_block(head, true)) {
                    let end = self.list[j].edit.text.len();
                    self.focus(j, end);
                }
                Outcome::Handled
            }
            Key::Up | Key::Down if m.cmd && m.shift => changed(self.move_by(range, ev.key == Key::Down)),
            Key::Up | Key::Down if plain => {
                let next = if ev.key == Key::Down { (head + 1).min(self.list.len() - 1) } else { head.saturating_sub(1) };
                let id = self.list[next].id;
                self.select(if m.shift { sel.anchor } else { id }, id);
                Outcome::Handled
            }
            Key::Backspace | Key::Delete => {
                self.selection = None;
                self.delete_range(range);
                Outcome::Changed
            }
            Key::Tab if plain => changed(self.nest_range(range, if m.shift { -1 } else { 1 })),
            _ if ev.is_command('c') || ev.is_command('x') => {
                tx.copy(markdown::write(&self.list[range.clone()]));
                if ev.is_command('x') {
                    self.selection = None;
                    self.delete_range(range);
                    Outcome::Changed
                } else {
                    Outcome::Handled
                }
            }
            _ if ev.is_command('d') => {
                self.duplicate(range);
                Outcome::Changed
            }
            _ if ev.is_command('a') => {
                self.select(self.list[0].id, self.list[self.list.len() - 1].id);
                Outcome::Handled
            }
            _ if ev.is_command('z') => changed(if m.shift { self.redo() } else { self.undo() }),
            Key::Text(_) => Outcome::Handled,
            _ => Outcome::Ignored,
        };
        if out == Outcome::Changed {
            if let Some(h) = self.selection.and_then(|s| self.index(s.head)) {
                self.keep_keys(h);
            }
            self.commit(None);
        }
        out
    }

    /// A press placed a caret in block `id`: that ends a block selection, and a menu elsewhere.
    pub(super) fn pressed(&mut self, id: u64) {
        self.selection = None;
        if self.menu.is_some_and(|m| m.block != id) {
            self.menu = None;
        }
    }

    /// Block `id`'s text changed in its field (typing, IME, cut, a paste it took).
    pub(super) fn typed(&mut self, id: u64) {
        if let Some(i) = self.index(id) {
            self.update_menu(i);
        }
        self.commit(Some(id));
    }

    /// Pastes `content` into block `id` as blocks when it holds more than plain text on one
    /// line; whether it did (the field pastes plain text otherwise).
    pub(super) fn paste(&mut self, id: u64, content: &ClipboardContent) -> bool {
        let Some(i) = self.index(id) else { return false };
        if !self.list[i].kind.is_rich() {
            return false;
        }
        let mut parts = markdown::parse(content.text());
        let plain = parts.len() == 1 && parts[0].0 == Kind::Paragraph && parts[0].3.is_empty();
        if parts.is_empty() || plain {
            return false;
        }
        let b = &mut self.list[i];
        b.edit.insert("");
        let at = b.edit.range().0;
        let ((head, head_marks), (tail, tail_marks)) = inline::split(&b.edit, at);
        let (indent, kind) = (b.indent, b.kind.clone());
        let first = parts.remove(0);
        // The first pasted block joins the text before the caret; the text after it follows the
        // last.
        let head_edit = model::field(&kind, &head, head_marks);
        let first_edit = model::field(&first.0, &first.2, first.3);
        let (text, marks) = inline::join(&head_edit, &first_edit);
        let first_kind = if head.is_empty() { first.0 } else { kind.clone() };
        let b = &mut self.list[i];
        b.kind = first_kind.clone();
        b.edit = model::field(&first_kind, &text, marks);
        let mut last = i;
        for (kind, by, text, marks) in parts {
            let block = self.make(kind, indent + by, &text, marks);
            last += 1;
            self.list.insert(last, block);
        }
        let caret = self.list[last].edit.text.len();
        let last_edit = self.list[last].edit.clone();
        let tail_edit = model::field(&kind, &tail, tail_marks);
        let (text, marks) = inline::join(&last_edit, &tail_edit);
        self.list[last].edit = model::field(&self.list[last].kind, &text, marks);
        self.focus(last, caret);
        self.commit(None);
        true
    }
}
