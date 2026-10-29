//! The document's structure: making, finding, focusing, splitting, merging, nesting and moving
//! blocks.

use std::ops::Range;

use rune::{Block as Line, MarkRange, TextEdit};

use super::{Block, Blocks, Kind, MAX_INDENT, Selection, inline, rich_marks};

impl Blocks {
    /// A block with a fresh id and a field set up for `kind`.
    pub(super) fn make(&mut self, kind: Kind, indent: u8, text: &str, marks: Vec<MarkRange>) -> Block {
        let id = self.next_id;
        self.next_id += 1;
        Block { id, edit: field(&kind, text, marks), kind, indent: indent.min(MAX_INDENT) }
    }

    pub(super) fn index(&self, id: u64) -> Option<usize> {
        self.list.iter().position(|b| b.id == id)
    }

    /// Block `id`'s field, or a scratch one when it's gone (a field of the last frame can
    /// outlive its block by an event).
    pub(super) fn edit_mut(&mut self, id: u64) -> &mut TextEdit {
        match self.list.iter_mut().find(|b| b.id == id) {
            Some(b) => &mut b.edit,
            None => &mut self.scratch,
        }
    }

    /// The block whose field has focus.
    pub(super) fn focused(&self) -> Option<usize> {
        self.list.iter().position(|b| b.edit.focused)
    }

    /// Focuses block `i`'s field with its caret at byte `caret`.
    pub(super) fn focus(&mut self, i: usize, caret: usize) {
        for (j, b) in self.list.iter_mut().enumerate() {
            b.edit.focused = j == i;
        }
        self.list[i].edit.set_caret(caret);
    }

    /// Block `i` and the blocks nested under it.
    pub(super) fn subtree(&self, i: usize) -> Range<usize> {
        let indent = self.list[i].indent;
        let end = self.list[i + 1..].iter().position(|b| b.indent <= indent).map_or(self.list.len(), |n| i + 1 + n);
        i..end
    }

    /// The selected blocks with the blocks nested under the last.
    pub(super) fn selected(&self) -> Option<Range<usize>> {
        let Selection { anchor, head } = self.selection?;
        let (a, b) = (self.index(anchor)?, self.index(head)?);
        Some(a.min(b)..self.subtree(a.max(b)).end)
    }

    /// The number a numbered item shows: one more than the numbered items right above it at its
    /// level.
    pub(super) fn number(&self, i: usize) -> usize {
        let indent = self.list[i].indent;
        let mut n = 1;
        for b in self.list[..i].iter().rev() {
            if b.indent > indent {
                continue;
            }
            if b.indent < indent || b.kind != Kind::Numbered {
                break;
            }
            n += 1;
        }
        n
    }

    /// Keeps a block that holds text, so the caret always has somewhere to go.
    pub(super) fn ensure_text_block(&mut self) {
        if !self.list.iter().any(|b| b.kind.has_text()) {
            let block = self.make(Kind::Paragraph, 0, "", Vec::new());
            self.list.push(block);
        }
    }

    /// Inserts a block of `kind` at `at` and focuses it, its caret at the start.
    pub(super) fn insert(&mut self, at: usize, kind: Kind, indent: u8, text: &str, marks: Vec<MarkRange>) {
        let block = self.make(kind, indent, text, marks);
        self.list.insert(at, block);
        self.focus(at, 0);
    }

    /// Return in block `i`: splits it at the caret, or ends the list an empty item is in.
    pub(super) fn enter(&mut self, i: usize) {
        let b = &mut self.list[i];
        let (a, z) = b.edit.range();
        if a != z {
            b.edit.insert("");
        }
        let empty = b.edit.text.is_empty();
        if empty && (b.kind.is_list() || b.kind == Kind::Quote) {
            if b.indent > 0 {
                self.nest(i, -1);
            } else {
                self.turn_into(i, Kind::Paragraph);
            }
            return;
        }
        let next = match &b.kind {
            Kind::Todo { .. } => Kind::Todo { checked: false },
            Kind::Toggle { .. } => Kind::Toggle { open: false },
            kind if kind.is_list() => kind.clone(),
            _ => Kind::Paragraph,
        };
        if a == 0 && !empty {
            // At the start, an empty block goes above and the caret stays with the text.
            let indent = b.indent;
            let kind = if b.kind.is_list() { next } else { Kind::Paragraph };
            self.insert(i, kind, indent, "", Vec::new());
            self.focus(i + 1, 0);
            return;
        }
        let ((text, marks), (tail, tail_marks)) = inline::split(&b.edit, a);
        let subtree = self.subtree(i);
        let b = &mut self.list[i];
        b.edit = field(&b.kind, &text, marks);
        // The text after the caret goes first inside an open toggle, after a closed one's
        // hidden blocks, and first among a block's children.
        let (at, indent, next) = match b.kind {
            Kind::Toggle { open: true } => (i + 1, b.indent + 1, Kind::Paragraph),
            Kind::Toggle { open: false } => (subtree.end, b.indent, next),
            _ => (i + 1, b.indent + u8::from(subtree.len() > 1), next),
        };
        self.insert(at, next, indent.min(MAX_INDENT), &tail, tail_marks);
    }

    /// Whether a closed toggle above block `i` hides it.
    pub(super) fn is_hidden(&self, i: usize) -> bool {
        let mut indent = self.list[i].indent;
        for b in self.list[..i].iter().rev() {
            if indent == 0 {
                return false;
            }
            if b.indent < indent {
                if matches!(b.kind, Kind::Toggle { open: false }) {
                    return true;
                }
                indent = b.indent;
            }
        }
        false
    }

    /// Which blocks closed toggles hide.
    pub(super) fn hidden(&self) -> Vec<bool> {
        let mut closed: Option<u8> = None;
        self.list
            .iter()
            .map(|b| {
                if closed.is_some_and(|c| b.indent <= c) {
                    closed = None;
                }
                let hidden = closed.is_some();
                if !hidden && matches!(b.kind, Kind::Toggle { open: false }) {
                    closed = Some(b.indent);
                }
                hidden
            })
            .collect()
    }

    /// Opens or closes toggle `i`; closing it takes the caret out of the blocks it hides.
    pub(super) fn toggle_open(&mut self, i: usize) -> bool {
        let Kind::Toggle { open } = &mut self.list[i].kind else { return false };
        *open = !*open;
        if !*open && self.focused().is_some_and(|f| self.subtree(i).contains(&f) && f != i) {
            let end = self.list[i].edit.text.len();
            self.focus(i, end);
        }
        true
    }

    /// Backspace at the start of block `i`: a list item, heading or quote becomes a
    /// paragraph, a nested paragraph moves out, and a paragraph merges into the block above.
    pub(super) fn backspace_at_start(&mut self, i: usize) -> bool {
        let b = &self.list[i];
        match &b.kind {
            Kind::Code { .. } if !b.edit.text.is_empty() => false,
            Kind::Paragraph if b.indent > 0 => self.nest(i, -1),
            Kind::Paragraph => self.merge_up(i),
            _ => {
                self.turn_into(i, Kind::Paragraph);
                true
            }
        }
    }

    /// Merges block `i` into the block shown above it (a divider above goes instead); the
    /// blocks nested under it stay nested under that one.
    fn merge_up(&mut self, i: usize) -> bool {
        let Some(p) = (0..i).rev().find(|&j| !self.is_hidden(j)) else { return false };
        if self.list[p].kind == Kind::Divider {
            self.list.remove(p);
            return true;
        }
        let gone = self.list.remove(i);
        self.tidy_indents(i);
        let prev = &mut self.list[p];
        let caret = prev.edit.text.len();
        let (text, marks) = inline::join(&prev.edit, &gone.edit);
        prev.edit = field(&prev.kind, &text, marks);
        self.focus(p, caret);
        true
    }

    /// Delete at the end of block `i`: the block shown below merges into it (a divider goes).
    pub(super) fn delete_at_end(&mut self, i: usize) -> bool {
        let Some(next) = (i + 1..self.list.len()).find(|&j| !self.is_hidden(j)) else { return false };
        if self.list[next].kind == Kind::Divider {
            self.list.remove(next);
            self.tidy_indents(next);
        } else {
            let caret = self.list[i].edit.text.len();
            self.merge_up(next);
            self.focus(i, caret);
        }
        true
    }

    /// Nests blocks `range` one level deeper (`by` 1) or shallower (-1), as far as the block
    /// above allows.
    pub(super) fn nest_range(&mut self, range: Range<usize>, by: i8) -> bool {
        let first = range.start;
        let limit = if first == 0 { 0 } else { (self.list[first - 1].indent + 1).min(MAX_INDENT) };
        let from = self.list[first].indent;
        let to = (i16::from(from) + i16::from(by)).clamp(0, i16::from(limit)) as u8;
        if to == from {
            return false;
        }
        for b in &mut self.list[range] {
            b.indent = (i16::from(b.indent) + i16::from(to) - i16::from(from)).clamp(0, i16::from(MAX_INDENT)) as u8;
        }
        true
    }

    /// Nests block `i` with its children.
    pub(super) fn nest(&mut self, i: usize, by: i8) -> bool {
        self.nest_range(self.subtree(i), by)
    }

    /// Moves blocks `range` to before block `to` (an index into the document as it is, outside
    /// `range`, or its length for the end), nested `indent` levels deeper or shallower.
    pub(super) fn move_range(&mut self, range: Range<usize>, to: usize, indent: i8) {
        let taken: Vec<Block> = self.list.drain(range.clone()).collect();
        let to = if to > range.start { to - range.len() } else { to };
        let limit = if to == 0 { 0 } else { (self.list[to - 1].indent + 1).min(MAX_INDENT) };
        let from = taken[0].indent;
        let first = (i16::from(from) + i16::from(indent)).clamp(0, i16::from(limit));
        let by = first - i16::from(from);
        let n = taken.len();
        self.list.splice(
            to..to,
            taken.into_iter().map(|mut b| {
                b.indent = (i16::from(b.indent) + by).clamp(0, i16::from(MAX_INDENT)) as u8;
                b
            }),
        );
        self.tidy_indents(to + n);
    }

    /// Moves block `i` (with its children, or the selection) above the block before it, or
    /// below the one after.
    pub(super) fn move_by(&mut self, range: Range<usize>, down: bool) -> bool {
        let indent = self.list[range.start].indent;
        if down {
            let Some(next) = (range.end < self.list.len()).then_some(range.end) else { return false };
            let after = self.subtree(next).end;
            self.move_range(range, after, 0);
        } else {
            let Some(prev) = self.list[..range.start].iter().rposition(|b| b.indent <= indent) else { return false };
            self.move_range(range, prev, 0);
        }
        true
    }

    /// Lifts blocks from `from` on that a move left nested deeper than the block above allows.
    fn tidy_indents(&mut self, from: usize) {
        for i in from.max(1)..self.list.len() {
            let limit = self.list[i - 1].indent + 1;
            if self.list[i].indent > limit {
                self.list[i].indent = limit;
            } else {
                break;
            }
        }
    }

    /// Turns block `i` into `kind`, keeping its text (a divider drops it, and the caret moves
    /// on to the block after, made if need be).
    pub(super) fn turn_into(&mut self, i: usize, kind: Kind) {
        let b = &mut self.list[i];
        let caret = b.edit.range().1;
        if kind == Kind::Divider {
            b.kind = Kind::Divider;
            b.edit = field(&Kind::Divider, "", Vec::new());
            let indent = b.indent;
            if self.list.get(i + 1).is_some_and(|b| b.kind.has_text()) {
                self.focus(i + 1, 0);
            } else {
                self.insert(i + 1, Kind::Paragraph, indent, "", Vec::new());
            }
            return;
        }
        if b.kind.is_rich() != kind.is_rich() {
            let marks = if kind.is_rich() { Vec::new() } else { b.edit.marks().to_vec() };
            let text = b.edit.text.clone();
            b.edit = field(&kind, &text, if kind.is_rich() { marks } else { Vec::new() });
        }
        b.kind = kind;
        self.focus(i, caret);
    }

    /// Deletes blocks `range`, focusing the end of the block above.
    pub(super) fn delete_range(&mut self, range: Range<usize>) {
        let start = range.start;
        self.list.drain(range);
        self.ensure_text_block();
        self.tidy_indents(start);
        let at = start.saturating_sub(1).min(self.list.len() - 1);
        let at = (0..=at)
            .rev()
            .find(|&j| self.list[j].kind.has_text())
            .or_else(|| self.list.iter().position(|b| b.kind.has_text()))
            .unwrap_or(0);
        let end = self.list[at].edit.text.len();
        self.focus(at, end);
    }

    /// Copies blocks `range` right after it, and selects the copies.
    pub(super) fn duplicate(&mut self, range: Range<usize>) {
        let copies: Vec<Block> = self.list[range.clone()]
            .iter()
            .cloned()
            .map(|mut b| {
                b.id = self.next_id;
                self.next_id += 1;
                b.edit.focused = false;
                b
            })
            .collect();
        let (first, last) = (copies[0].id, copies[copies.len() - 1].id);
        self.list.splice(range.end..range.end, copies);
        self.selection = Some(Selection { anchor: first, head: last });
    }

    pub(super) fn toggle_todo(&mut self, i: usize) {
        if let Kind::Todo { checked } = &mut self.list[i].kind {
            *checked = !*checked;
        }
    }
}

/// A block's field: rich text (with Markdown's marks) for text kinds, plain for code.
pub(super) fn field(kind: &Kind, text: &str, marks: Vec<MarkRange>) -> TextEdit {
    let lines = text.split('\n').count();
    let edit = TextEdit::from_parts(text, marks, vec![Line::Paragraph; lines]).allow_blocks([Line::Paragraph]);
    if kind.is_rich() {
        edit.allow_marks(rich_marks())
    } else {
        let mut edit = edit.allow_marks([]);
        edit.set_rich(false);
        edit
    }
}
