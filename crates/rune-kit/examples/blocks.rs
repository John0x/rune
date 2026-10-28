//! A Notion-like block editor over a Markdown note.
//!
//! Run it, or headless: `cargo run -p rune-kit --features blocks --example blocks -- --snap blocks.png`.
//! F2 (or `--scene markdown`) shows the note as the Markdown the editor writes.

use rune::{AppSpec, Cx, El, Key, KeyEvent, ScrollSpec, Tx, col, text};
use rune_kit::blocks::{BlockEditor, Blocks, GUTTER};
use rune_kit::{Kit, root};

const NOTE: &str = "# Weekly sync

## Updates and wins

- Shipped the **new onboarding** to *everyone*
    - Activation up `12%` week over week
- Hired a designer
- [ ] Send the [launch post](https://example.com/launch) to the team
- [x] Book the offsite

## Challenges and blockers

> The importer still drops nested lists.

1. Reproduce it
2. Write the fix

```rust
fn main() {
    println!(\"hello\");
}
```

---

Next milestone: the public beta.
";

struct State {
    note: Blocks,
    markdown: bool,
}

impl Default for State {
    fn default() -> Self {
        State { note: Blocks::from_markdown(NOTE), markdown: false }
    }
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let k = Kit::neutral(true);
    let mut layer = Vec::new();
    let body = if s.markdown {
        text(s.note.to_markdown()).font(k.ty.mono).color(k.c.text_base).selectable(true)
    } else {
        BlockEditor::new("note", &s.note)
            .placeholder("Write something, or press / for commands")
            .on_edit(|s: &mut State| &mut s.note)
            .build(&k, cx, &mut layer)
    };
    let page = col()
        .key("page")
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(k.c.page)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().key("column").w_full().max_w(760.0 + GUTTER).self_center().pt(40.0).pr(24.0).child(body));
    root(page, layer)
}

fn key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    let toggle = ev.key == Key::Function(2);
    if toggle {
        s.markdown = !s.markdown;
    }
    toggle
}

fn main() {
    rune::run(AppSpec {
        fonts: &[
            include_bytes!("../../rune/assets/Inter-Regular.ttf"),
            include_bytes!("../../rune/assets/Inter-Medium.ttf"),
            include_bytes!("../../rune/assets/Inter-SemiBold.ttf"),
            include_bytes!("../../rune/assets/IBMPlexMono-Regular.ttf"),
        ],
        families: ("Inter", "IBM Plex Mono"),
        scenes: &[("markdown", |s: &mut State| s.markdown = true)],
        on_key: Some(key),
        ..AppSpec::new("Blocks", (860.0, 760.0), State::default, view)
    });
}
