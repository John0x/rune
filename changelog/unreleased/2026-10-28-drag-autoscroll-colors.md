- **`El::drag_autoscroll()`: a drag scrolls the view it's held at the edge of.** While the
  element's `on_drag` holds the pointer within 48 points of the top or bottom of the scroll
  viewport around it (or, for an element around a page, the one inside it under the pointer),
  the viewport scrolls, faster nearer the edge, and the drag gets a `Phase::Move` each frame.
- **`Mark::Color(n)` and `Mark::Highlight(n)`: colored text and backgrounds in rich text.**
  They draw in `RichStyle::colors` and `RichStyle::highlights` (Notion's gray, brown, orange,
  yellow, green, blue, purple, pink and red by default; rune-kit's fields use Notion's own for
  light and dark), merge and replace like links, and stay out of HTML.
