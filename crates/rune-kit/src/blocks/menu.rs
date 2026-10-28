//! The slash menu: `/` typed at a block's start or after a space lists the kinds of block,
//! filtered by what follows it, to turn the block into one (or add one below it).

use super::{Blocks, Kind};

/// The open menu: the block it's in, where its `/` is, and the highlighted choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SlashMenu {
    pub(super) block: u64,
    pub(super) slash: usize,
    pub(super) active: usize,
}

/// A kind the menu offers.
pub(super) struct Choice {
    pub(super) id: &'static str,
    /// The Markdown shortcut that makes it, shown beside its name.
    pub(super) shortcut: &'static str,
    words: &'static str,
}

pub(super) const CHOICES: [Choice; 10] = [
    Choice { id: "text", shortcut: "", words: "text paragraph plain" },
    Choice { id: "h1", shortcut: "#", words: "heading 1 h1 title" },
    Choice { id: "h2", shortcut: "##", words: "heading 2 h2 subtitle" },
    Choice { id: "h3", shortcut: "###", words: "heading 3 h3" },
    Choice { id: "bullet", shortcut: "-", words: "bulleted list bullet ul unordered" },
    Choice { id: "numbered", shortcut: "1.", words: "numbered list ordered ol" },
    Choice { id: "todo", shortcut: "[]", words: "to-do list todo task checkbox" },
    Choice { id: "quote", shortcut: ">", words: "quote blockquote citation" },
    Choice { id: "code", shortcut: "```", words: "code snippet block" },
    Choice { id: "divider", shortcut: "---", words: "divider rule separator line hr" },
];

impl Choice {
    pub(super) fn kind(&self) -> Kind {
        match self.id {
            "h1" => Kind::Heading(1),
            "h2" => Kind::Heading(2),
            "h3" => Kind::Heading(3),
            "bullet" => Kind::Bullet,
            "numbered" => Kind::Numbered,
            "todo" => Kind::Todo { checked: false },
            "quote" => Kind::Quote,
            "code" => Kind::Code { language: String::new() },
            "divider" => Kind::Divider,
            _ => Kind::Paragraph,
        }
    }

    fn matches(&self, query: &str) -> bool {
        let label = self.kind().label().to_lowercase();
        query.split_whitespace().all(|q| label.contains(q) || self.words.split(' ').any(|w| w.starts_with(q)))
    }
}

/// The choices `query` (lowercased) matches.
pub(super) fn matching(query: &str) -> Vec<&'static Choice> {
    let query = query.to_lowercase();
    CHOICES.iter().filter(|c| c.matches(&query)).collect()
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
                text.as_bytes().get(m.slash) == Some(&b'/') && q.len() <= 24 && !q.contains('\n') && !(q.ends_with(' ') && matching(q).is_empty())
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
        let n = self.menu_query().map_or(0, |(_, q)| matching(q).len()) as isize;
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
        let choices = matching(query);
        let choice = match id {
            Some(id) => CHOICES.iter().find(|c| c.id == id),
            None => choices.get(menu.active).copied(),
        };
        let (block, slash) = (menu.block, menu.slash);
        self.menu = None;
        let (Some(choice), Some(i)) = (choice, self.index(block)) else { return false };
        let edit = &mut self.list[i].edit;
        let caret = edit.range().1;
        edit.set_selection(slash, caret);
        edit.insert("");
        let kind = choice.kind();
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
