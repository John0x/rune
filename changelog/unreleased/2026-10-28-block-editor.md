- **`rune_kit::blocks`: a Notion-like block editor** (feature `blocks`). A `Blocks` document of
  paragraphs, headings, bulleted, numbered and to-do lists, quotes, code and dividers, each its
  own rich text field, edited by a `BlockEditor`. Markdown shortcuts change a block's kind as you
  type (`# `, `- `, `1. `, `[] `, `> `, ```` ``` ````, `---`) and format text as you close them
  (`**bold**`, `*italic*`, `` `code` ``, `~~strike~~`, `[link](url)`); `/` opens a menu of kinds.
  Return, Backspace, Tab and the arrows work across blocks; Escape, Shift+arrows or a text drag
  that leaves its block select blocks whole to move (⌘⇧↑↓), nest, duplicate, copy as Markdown or
  delete. A hovered block's handle drags it with its children, sideways to nest it. Undo spans
  the document, which reads and writes Markdown (`from_markdown`, `to_markdown`,
  `set_markdown`). `examples/blocks` shows it. For it, text fields gained `Tx::place_caret` (focus
  a field at a point, as a click), `El::on_input_press` and `El::on_input_drag`.
