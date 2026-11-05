//! The slash menu: `/` typed at a block's start or after a space lists the kinds of block,
//! filtered by what follows it, to turn the block into one (or add one below it).

use rune::Icon;

use super::{Blocks, Kind};

/// The open menu: the block it's in, where its `/` is, and the highlighted choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SlashMenu {
    pub(super) block: u64,
    pub(super) slash: usize,
    pub(super) active: usize,
}

/// What a slash menu's row does.
#[derive(Clone, Copy, Debug)]
pub enum SlashDoes {
    /// Turns the block into this kind, or adds one below it.
    Turn(fn() -> Kind),
    /// The app's: [`BlockEditor::on_slash`](super::BlockEditor::on_slash) hears its id.
    Action,
}

/// A row of the slash menu.
#[derive(Clone, Copy, Debug)]
pub struct SlashItem {
    pub id: &'static str,
    pub label: &'static str,
    /// Its mark, before its label.
    pub icon: Option<&'static Icon>,
    /// What's shown on its end: the keys or Markdown shortcut that make it.
    pub shortcut: &'static str,
    /// More words it's found by.
    pub words: &'static str,
    pub does: SlashDoes,
    /// A line above it, starting a group.
    pub group_start: bool,
}

impl SlashItem {
    const fn turn(id: &'static str, label: &'static str, shortcut: &'static str, words: &'static str, kind: fn() -> Kind) -> Self {
        SlashItem { id, label, icon: None, shortcut, words, does: SlashDoes::Turn(kind), group_start: false }
    }

    /// What it turns a block into (a row of the app's: a paragraph).
    pub(super) fn kind(&self) -> Kind {
        match self.does {
            SlashDoes::Turn(kind) => kind(),
            SlashDoes::Action => Kind::Paragraph,
        }
    }

    fn matches(&self, query: &str) -> bool {
        let label = self.label.to_lowercase();
        query.split_whitespace().all(|q| label.contains(q) || self.words.split(' ').any(|w| w.starts_with(q)))
    }
}

/// What the menu offers unless the app says otherwise ([`Blocks::set_slash_items`]).
pub(super) const CHOICES: [SlashItem; 11] = [
    SlashItem::turn("text", "Text", "", "text paragraph plain", || Kind::Paragraph),
    SlashItem::turn("h1", "Heading 1", "#", "heading 1 h1 title", || Kind::Heading(1)),
    SlashItem::turn("h2", "Heading 2", "##", "heading 2 h2 subtitle", || Kind::Heading(2)),
    SlashItem::turn("h3", "Heading 3", "###", "heading 3 h3", || Kind::Heading(3)),
    SlashItem::turn("bullet", "Bulleted list", "-", "bulleted list bullet ul unordered", || Kind::Bullet),
    SlashItem::turn("numbered", "Numbered list", "1.", "numbered list ordered ol", || Kind::Numbered),
    SlashItem::turn("todo", "To-do list", "[]", "to-do list todo task checkbox", || Kind::Todo { checked: false }),
    SlashItem::turn("toggle", "Toggle list", ">", "toggle list collapse expand details", || Kind::Toggle { open: false }),
    SlashItem::turn("quote", "Quote", "\"", "quote blockquote citation", || Kind::Quote),
    SlashItem::turn("code", "Code", "```", "code snippet block", || Kind::Code { language: String::new() }),
    SlashItem::turn("divider", "Divider", "---", "divider rule separator line hr", || Kind::Divider),
];

/// Those of `items` that `query` (lowercased) matches.
pub(super) fn matching(items: &'static [SlashItem], query: &str) -> Vec<&'static SlashItem> {
    let query = query.to_lowercase();
    items.iter().filter(|c| c.matches(&query)).collect()
}

impl Blocks {
    /// The menu's query: what follows its `/` up to the caret, while the menu is open.
    pub(super) fn menu_query(&self) -> Option<(&SlashMenu, &str)> {
        let menu = self.menu.as_ref()?;
        let b = &self.list[self.index(menu.block)?];
        let caret = b.edit.range().1;
        b.edit.text.get(menu.slash + 1..caret).map(|q| (menu, q))
    }

    /// Opens, filters or closes the menu after block `i`'s text changed.
    pub(super) fn update_menu(&mut self, i: usize) {
        let b = &self.list[i];
        let (a, z) = b.edit.range();
        let text = &b.edit.text;
        if self.menu.is_some_and(|m| m.block == b.id) {
            let keep = self.menu_query().is_some_and(|(m, q)| {
                text.as_bytes().get(m.slash) == Some(&b'/')
                    && q.len() <= 24
                    && !q.contains('\n')
                    && !(q.ends_with(' ') && matching(self.slash, q).is_empty())
            });
            if !keep {
                self.menu = None;
            } else if let Some(m) = &mut self.menu {
                m.active = 0;
            }
            return;
        }
        let typed_slash = a == z && a > 0 && text.as_bytes()[a - 1] == b'/';
        if typed_slash && (a == 1 || text[..a - 1].ends_with(char::is_whitespace)) && b.kind.is_rich() {
            self.menu = Some(SlashMenu { block: b.id, slash: a - 1, active: 0 });
        }
    }

    /// Moves the highlight `by` choices, wrapping.
    pub(super) fn menu_step(&mut self, by: isize) {
        let n = self.menu_query().map_or(0, |(_, q)| matching(self.slash, q).len()) as isize;
        if let Some(m) = &mut self.menu
            && n > 0
        {
            m.active = (m.active as isize + by).rem_euclid(n) as usize;
        }
    }

    /// Takes the highlighted choice, or the one with `id`.
    pub(super) fn menu_choose(&mut self, id: Option<&str>) -> bool {
        let Some((menu, query)) = self.menu_query() else {
            self.menu = None;
            return false;
        };
        let choices = matching(self.slash, query);
        let choice = match id {
            Some(id) => self.slash.iter().find(|c| c.id == id),
            None => choices.get(menu.active).copied(),
        };
        let (block, slash) = (menu.block, menu.slash);
        self.menu = None;
        let (Some(choice), Some(i)) = (choice, self.index(block)) else { return false };
        let edit = &mut self.list[i].edit;
        let caret = edit.range().1;
        edit.set_selection(slash, caret);
        edit.insert("");
        let SlashDoes::Turn(kind) = choice.does else {
            self.slash_action = Some(choice.id);
            return true;
        };
        let kind = kind();
        if self.list[i].edit.text.is_empty() {
            self.turn_into(i, kind);
        } else {
            let indent = self.list[i].indent;
            let at = self.subtree(i).end;
            self.insert(at, kind.clone(), indent, "", Vec::new());
            if kind == Kind::Divider {
                self.turn_into(at, Kind::Divider);
            }
        }
        true
    }
}
