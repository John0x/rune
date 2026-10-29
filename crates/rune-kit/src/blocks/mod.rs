//! A block editor, as Notion's: a document of blocks (paragraphs, headings, lists, to-dos,
//! quotes, code, dividers), each its own rich text field.
//!
//! Markdown shortcuts turn a block into another kind as you type at its start (`# `, `## `,
//! `### `, `- `, `1. `, `[] `, `> ` for a toggle, `" ` for a quote, ```` ``` ````, `---`) and
//! format text as you close them (`**bold**`, `*italic*`, `` `code` ``, `~~strike~~`,
//! `[link](url)`); `/` opens a menu of kinds. Selecting text shows a toolbar to format it,
//! link it (⌘K), color it or turn its block into another kind.
//! Return splits a block, Backspace at its start turns it back into a paragraph and then merges
//! it into the one above, Tab and Shift+Tab nest it; the arrows move between blocks and keep
//! the caret's column. Escape selects the block, Shift and the arrows extend the selection, and
//! a selection moves (⌘⇧↑, ⌘⇧↓), nests, duplicates (⌘D), copies, cuts or deletes blocks whole.
//! A block's handle, shown on hover, drags it with the blocks nested under it, sideways to nest
//! it, and opens its menu when clicked. A toggle hides the blocks nested under it while closed.
//! Undo and redo cover the whole document. A document reads and writes Markdown.
//!
//! The app owns a [`Blocks`] and binds a [`BlockEditor`] to it, mounting the slash menu's
//! layer at its root like any overlay:
//!
//! ```ignore
//! struct App { note: Blocks }
//!
//! fn view(s: &App, cx: &mut Cx<App>) -> El {
//!     let k = Kit::neutral(true);
//!     let mut layer = Vec::new();
//!     let editor = BlockEditor::new("note", &s.note)
//!         .placeholder("Write something, or press / for commands")
//!         .on_edit(|s: &mut App| &mut s.note)
//!         .on_change(|s: &mut App, _| s.save())
//!         .build(&k, cx, &mut layer);
//!     root(col().child(editor), layer)
//! }
//! ```

mod handle;
mod history;
mod inline;
mod keys;
mod markdown;
mod marquee;
mod menu;
mod model;
mod toolbar;
mod view;

use rune::{Mark, MarkRange, TextEdit};

pub use handle::GUTTER;
pub use marquee::selection_area;
pub use view::BlockEditor;

/// What a block is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Paragraph,
    /// A heading of level 1 to 3.
    Heading(u8),
    Bullet,
    Numbered,
    Todo {
        checked: bool,
    },
    Quote,
    /// A toggle list item: the blocks nested under it show while it's open.
    Toggle {
        open: bool,
    },
    /// A code block; `language` names its syntax (`rust`, `ts`), or is empty.
    Code {
        language: String,
    },
    Divider,
}

impl Kind {
    /// Its name in the slash menu.
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Paragraph => "Text",
            Kind::Heading(1) => "Heading 1",
            Kind::Heading(2) => "Heading 2",
            Kind::Heading(_) => "Heading 3",
            Kind::Bullet => "Bulleted list",
            Kind::Numbered => "Numbered list",
            Kind::Todo { .. } => "To-do list",
            Kind::Quote => "Quote",
            Kind::Toggle { .. } => "Toggle list",
            Kind::Code { .. } => "Code",
            Kind::Divider => "Divider",
        }
    }

    /// A list item: Return continues it with another of its kind.
    pub fn is_list(&self) -> bool {
        matches!(self, Kind::Bullet | Kind::Numbered | Kind::Todo { .. } | Kind::Toggle { .. })
    }

    /// Whether it holds text (all but dividers).
    pub fn has_text(&self) -> bool {
        !matches!(self, Kind::Divider)
    }

    /// Whether its text takes inline formatting (all but code and dividers).
    pub fn is_rich(&self) -> bool {
        !matches!(self, Kind::Code { .. } | Kind::Divider)
    }
}

/// One block of a [`Blocks`] document: its kind, how deep it's nested, and its text.
#[derive(Clone, Debug)]
pub struct Block {
    id: u64,
    pub kind: Kind,
    /// Nesting level: a block is a child of the nearest block above it with a smaller one.
    pub indent: u8,
    /// Its text and inline formatting (bold, italic, strike, code, links, colors).
    pub edit: TextEdit,
}

impl Block {
    /// Its id, stable for the life of the document (and across undo).
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn text(&self) -> &str {
        &self.edit.text
    }

    pub fn marks(&self) -> &[MarkRange] {
        self.edit.marks()
    }
}

/// The deepest nesting level.
pub const MAX_INDENT: u8 = 8;

/// A block document, owned by the app and edited by a [`BlockEditor`].
#[derive(Clone, Debug)]
pub struct Blocks {
    list: Vec<Block>,
    next_id: u64,
    /// Blocks selected whole, from `anchor` to `head` (ids), after Escape or a drag.
    selection: Option<Selection>,
    menu: Option<menu::SlashMenu>,
    drag: Option<handle::BlockDrag>,
    /// The block whose ⋮⋮ opened its menu, and where (window points).
    block_menu: Option<(u64, [f32; 2])>,
    /// Whether the block menu's colors show.
    block_colors: bool,
    /// A rectangle being dragged to select blocks.
    marquee: Option<marquee::Marquee>,
    hovered: Option<u64>,
    /// The block edited last, where a Shift-click's selection starts.
    last_block: Option<u64>,
    /// The formatting toolbar's open menu or link field.
    toolbar: toolbar::Toolbar,
    /// The color ⌘⇧H applies: the last one picked.
    last_color: Mark,
    history: history::History,
    revision: u64,
    /// What a field bound to a block that's gone edits.
    scratch: TextEdit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    anchor: u64,
    head: u64,
}

impl Default for Blocks {
    fn default() -> Self {
        Self::new()
    }
}

impl Blocks {
    /// A document of one empty paragraph.
    pub fn new() -> Blocks {
        Blocks::from_parts(vec![(Kind::Paragraph, 0, String::new(), Vec::new())])
    }

    /// The document `markdown` describes (CommonMark with GFM task lists and strikethrough).
    pub fn from_markdown(markdown: &str) -> Blocks {
        Blocks::from_parts(markdown::parse(markdown))
    }

    /// The document as Markdown.
    pub fn to_markdown(&self) -> String {
        markdown::write(&self.list)
    }

    /// Replaces the whole document with `markdown`, as one step undo takes back.
    pub fn set_markdown(&mut self, markdown: &str) {
        let parts = markdown::parse(markdown);
        self.list =
            parts.into_iter().map(|(kind, indent, text, marks)| self.make(kind, indent, &text, marks)).collect();
        self.ensure_text_block();
        self.selection = None;
        self.menu = None;
        self.commit(None);
    }

    fn from_parts(parts: Vec<(Kind, u8, String, Vec<MarkRange>)>) -> Blocks {
        let mut doc = Blocks {
            list: Vec::new(),
            next_id: 1,
            selection: None,
            menu: None,
            drag: None,
            block_menu: None,
            block_colors: false,
            marquee: None,
            hovered: None,
            last_block: None,
            toolbar: toolbar::Toolbar::default(),
            last_color: Mark::Highlight(3),
            history: history::History::default(),
            revision: 0,
            scratch: TextEdit::new(""),
        };
        doc.list = parts.into_iter().map(|(kind, indent, text, marks)| doc.make(kind, indent, &text, marks)).collect();
        doc.ensure_text_block();
        doc.history.reset(doc.snapshot());
        doc
    }

    pub fn blocks(&self) -> &[Block] {
        &self.list
    }

    /// Counts the document's changes: save it when this moves.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the document is one empty paragraph.
    pub fn is_empty(&self) -> bool {
        self.list.len() == 1 && self.list[0].kind == Kind::Paragraph && self.list[0].edit.text.is_empty()
    }

    /// Focuses the last block that holds text, its caret at the end, as clicking below the
    /// document does.
    pub fn focus_end(&mut self) {
        let last = self.list.iter().rposition(|b| b.kind.has_text()).unwrap_or(0);
        let end = self.list[last].edit.text.len();
        self.focus(last, end);
    }

    /// Focuses the first block that holds text, its caret at the start.
    pub fn focus_start(&mut self) {
        let first = self.list.iter().position(|b| b.kind.has_text()).unwrap_or(0);
        self.focus(first, 0);
    }

    /// Takes keyboard focus from the blocks, and drops a block selection and the menu.
    pub fn blur(&mut self) {
        for b in &mut self.list {
            b.edit.focused = false;
        }
        self.selection = None;
        self.menu = None;
    }

    /// Whether one of the blocks has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused().is_some()
    }
}

/// Bold, italic, underline, strike, inline code, links and colors.
fn rich_marks() -> [Mark; 8] {
    [
        Mark::Bold,
        Mark::Italic,
        Mark::Underline,
        Mark::Strike,
        Mark::Code,
        Mark::Link(String::new()),
        Mark::Color(0),
        Mark::Highlight(0),
    ]
}

/// Notion's colors, in [`Mark::Color`] and [`Mark::Highlight`]'s order.
pub const COLORS: [&str; 9] = ["Gray", "Brown", "Orange", "Yellow", "Green", "Blue", "Purple", "Pink", "Red"];
