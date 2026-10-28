//! Undo and redo over the whole document. Every change records the document after it; typing
//! in one block within a second of the last keystroke joins the step before, as a text field's
//! own undo groups a word.

use std::time::Duration;

use rune::MarkRange;
use rune::time::Instant;

use super::{Blocks, Kind, model};

/// How long a pause in typing ends an undo step.
const TYPING_PAUSE: Duration = Duration::from_secs(1);
/// Steps kept.
const DEPTH: usize = 200;

#[derive(Clone, Debug)]
struct Saved {
    id: u64,
    kind: Kind,
    indent: u8,
    text: String,
    marks: Vec<MarkRange>,
}

/// The document at one point: its blocks and where the caret was.
#[derive(Clone, Debug, Default)]
pub(super) struct Snapshot {
    blocks: Vec<Saved>,
    caret: Option<(u64, usize, usize)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The document as the last change left it.
    current: Snapshot,
    /// The block being typed in and when, while its keystrokes join one step.
    typing: Option<(u64, Instant)>,
}

impl History {
    pub(super) fn reset(&mut self, current: Snapshot) {
        *self = History { current, ..History::default() };
    }
}

impl Blocks {
    pub(super) fn snapshot(&self) -> Snapshot {
        let caret = self.focused().map(|i| {
            let b = &self.list[i];
            (b.id, b.edit.anchor, b.edit.focus)
        });
        let blocks = self
            .list
            .iter()
            .map(|b| Saved {
                id: b.id,
                kind: b.kind.clone(),
                indent: b.indent,
                text: b.edit.text.clone(),
                marks: b.edit.marks().to_vec(),
            })
            .collect();
        Snapshot { blocks, caret }
    }

    /// Records a change: one undo step, or part of the last one while `typing` in the same
    /// block.
    pub(super) fn commit(&mut self, typing: Option<u64>) {
        let now = Instant::now();
        let h = &mut self.history;
        let joins = typing.is_some()
            && h.typing.is_some_and(|(id, at)| Some(id) == typing && now.duration_since(at) < TYPING_PAUSE);
        let after = self.snapshot();
        let h = &mut self.history;
        if !joins {
            h.undo.push(std::mem::take(&mut h.current));
            if h.undo.len() > DEPTH {
                h.undo.remove(0);
            }
        }
        h.redo.clear();
        h.current = after;
        h.typing = typing.map(|id| (id, now));
        self.revision += 1;
    }

    pub(super) fn undo(&mut self) -> bool {
        let Some(before) = self.history.undo.pop() else { return false };
        let now = self.snapshot();
        self.history.redo.push(now);
        self.restore(before);
        true
    }

    pub(super) fn redo(&mut self) -> bool {
        let Some(after) = self.history.redo.pop() else { return false };
        let now = self.snapshot();
        self.history.undo.push(now);
        self.restore(after);
        true
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.list = snapshot
            .blocks
            .iter()
            .map(|s| super::Block { id: s.id, edit: model::field(&s.kind, &s.text, s.marks.clone()), kind: s.kind.clone(), indent: s.indent })
            .collect();
        self.selection = None;
        self.menu = None;
        match snapshot.caret.and_then(|(id, anchor, focus)| Some((self.index(id)?, anchor, focus))) {
            Some((i, anchor, focus)) => {
                self.focus(i, focus);
                self.list[i].edit.set_selection(anchor, focus);
            }
            None => self.focus_end(),
        }
        self.history.current = snapshot;
        self.history.typing = None;
        self.revision += 1;
    }
}
