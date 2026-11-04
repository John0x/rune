//! The `@` menu: `@` typed at a word's start lists what the app offers, filtered by what it's told
//! follows it; what's picked replaces them with a pill.

use super::Blocks;

/// The open menu: the block it's in, where its `@` is, and the highlighted choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MentionMenu {
    pub(super) block: u64,
    pub(super) at: usize,
    pub(super) active: usize,
}

/// What the `@` menu offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionItem {
    /// What the pill carries, in Markdown too (`[label](mention:id)`).
    pub id: String,
    pub label: String,
    /// Shown beside it, quietly.
    pub detail: Option<String>,
}

impl Blocks {
    /// What follows the `@` up to the caret, while the menu is open.
    pub fn mention_query(&self) -> Option<&str> {
        let menu = self.mention.as_ref()?;
        let b = &self.list[self.index(menu.block)?];
        let caret = b.edit.range().1;
        b.edit.text.get(menu.at + 1..caret)
    }

    /// What the `@` menu offers now.
    pub fn set_mention_items(&mut self, items: Vec<MentionItem>) {
        self.mention_items = items;
        if let Some(menu) = &mut self.mention {
            menu.active = menu.active.min(self.mention_items.len().saturating_sub(1));
        }
    }

    pub(super) fn mention_items(&self) -> &[MentionItem] {
        &self.mention_items
    }

    /// Opens, follows or closes the menu after block `i`'s text changed.
    pub(super) fn update_mention(&mut self, i: usize) {
        let b = &self.list[i];
        let (a, z) = b.edit.range();
        let text = &b.edit.text;
        if self.mention.is_some_and(|m| m.block == b.id) {
            let keep = self.mention_query().is_some_and(|q| !q.contains(char::is_whitespace) && q.len() <= 64);
            let holds = self.mention.is_some_and(|m| text.as_bytes().get(m.at) == Some(&b'@'));
            if !keep || !holds {
                self.mention = None;
            } else if let Some(m) = &mut self.mention {
                m.active = 0;
            }
            return;
        }
        let typed = a == z && a > 0 && text.as_bytes()[a - 1] == b'@';
        if typed && (a == 1 || text[..a - 1].ends_with(char::is_whitespace)) && b.kind.is_rich() {
            self.mention = Some(MentionMenu { block: b.id, at: a - 1, active: 0 });
            self.mention_items.clear();
        }
    }

    pub(super) fn mention_step(&mut self, by: isize) {
        let n = self.mention_items.len() as isize;
        if let Some(m) = &mut self.mention
            && n > 0
        {
            m.active = (m.active as isize + by).rem_euclid(n) as usize;
        }
    }

    /// Takes the highlighted choice, or the one with `id`: a pill, then a space.
    pub(super) fn mention_choose(&mut self, id: Option<&str>) -> bool {
        let Some(menu) = self.mention.take() else { return false };
        let item = match id {
            Some(id) => self.mention_items.iter().find(|item| item.id == id),
            None => self.mention_items.get(menu.active),
        };
        let (Some(item), Some(i)) = (item.cloned(), self.index(menu.block)) else { return false };
        let edit = &mut self.list[i].edit;
        let caret = edit.range().1;
        edit.replace_with_pill(menu.at..caret, item.id, &item.label);
        edit.insert(" ");
        true
    }
}
