- **`rune_kit::blocks`: a Notion-like block editor** (feature `blocks`). A `Blocks` document of
  paragraphs, headings, bulleted, numbered, to-do and toggle lists, quotes, code and dividers, each its
  own rich text field, edited by a `BlockEditor`. Markdown shortcuts change a block's kind as you
  type (`# `, `- `, `1. `, `[] `, `> ` a toggle, `" ` a quote, ```` ``` ````, `---`) and format text as you close them
  (`**bold**`, `*italic*`, `` `code` ``, `~~strike~~`, `[link](url)`); `/` opens a menu of kinds.
  Return, Backspace, Tab and the arrows work across blocks; Escape, Shift+arrows or a text drag
  that leaves its block select blocks whole to move (⌘⇧↑↓), nest, duplicate, copy as Markdown or
  delete; a drag that starts outside the text (or in the page around it, with
  `blocks::selection_area`) draws a rectangle selecting the blocks it crosses, as Notion's does, and Shift-click extends a selection. A hovered block's ⋮⋮ drags it with its children (or the selection it's part of): a faded copy follows the pointer, a line shows where it lands, sideways nests it, and the page scrolls at its edges; clicked, it opens the block's menu (color, duplicate, copy, delete, turn into). ⌘⌥0–8 turn the block or selection into text, a heading, a to-do, a list, a toggle or code. Selected text gets Notion's toolbar: turn into, bold, italic, underline, strike, code, a link (⌘K) and colors (⌘⇧H the last one); ⌘E and ⌘⇧S format too. A closed toggle hides the blocks nested under it. Undo spans
  the document, which reads and writes Markdown (`from_markdown`, `to_markdown`,
  `set_markdown`), `BlockEditor::decorate` styles ranges of a block's text (who wrote what), and `BlockEditor::aside` puts an element at the end of each row. `examples/blocks` shows it. For it, text fields gained `Tx::place_caret` (focus
  a field at a point, as a click), `El::on_input_press` and `El::on_input_drag`.
