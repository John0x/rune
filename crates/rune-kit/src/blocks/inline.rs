//! Text inside a block: splitting and joining fields with their formatting, and the Markdown
//! shortcuts typed at a block's start (`# `) or around text (`**bold**`).

use rune::{Mark, MarkRange, TextEdit};

use super::Kind;

/// `edit` cut at byte `at` into two texts with their marks.
pub(super) fn split(edit: &TextEdit, at: usize) -> ((String, Vec<MarkRange>), (String, Vec<MarkRange>)) {
    let text = &edit.text;
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    for m in edit.marks() {
        if m.start < at {
            head.push(MarkRange::new(m.start..m.end.min(at), m.mark.clone()));
        }
        if m.end > at {
            tail.push(MarkRange::new(m.start.max(at) - at..m.end - at, m.mark.clone()));
        }
    }
    ((text[..at].to_owned(), head), (text[at..].to_owned(), tail))
}

/// `a`'s text and marks followed by `b`'s.
pub(super) fn join(a: &TextEdit, b: &TextEdit) -> (String, Vec<MarkRange>) {
    let n = a.text.len();
    let mut marks = a.marks().to_vec();
    marks.extend(b.marks().iter().map(|m| MarkRange::new(m.start + n..m.end + n, m.mark.clone())));
    (format!("{}{}", a.text, b.text), marks)
}

/// The kind a paragraph's first characters ask for when a space follows them.
pub(super) fn block_prefix(prefix: &str) -> Option<Kind> {
    Some(match prefix {
        "#" => Kind::Heading(1),
        "##" => Kind::Heading(2),
        "###" => Kind::Heading(3),
        "-" | "*" | "+" => Kind::Bullet,
        "[]" | "[ ]" => Kind::Todo { checked: false },
        "[x]" | "[X]" => Kind::Todo { checked: true },
        ">" | "\"" => Kind::Quote,
        p if p.len() > 1 && p.ends_with('.') && p[..p.len() - 1].bytes().all(|b| b.is_ascii_digit()) => Kind::Numbered,
        _ => return None,
    })
}

/// Formatting that typing `typed` before byte `caret` of `text` closes, as `(opener, content,
/// closer typed so far, mark)` byte ranges: `**bold*` and a typed `*` close bold.
struct Closing {
    open: std::ops::Range<usize>,
    content: std::ops::Range<usize>,
    /// The part of the closing delimiter already in the text, before the caret.
    close: std::ops::Range<usize>,
    mark: Mark,
}

/// Whether `content` can be formatted: not empty, and not starting or ending with a space.
fn tight(content: &str) -> bool {
    !content.is_empty() && !content.starts_with(char::is_whitespace) && !content.ends_with(char::is_whitespace)
}

fn closing(text: &str, caret: usize, typed: char) -> Option<Closing> {
    let before = &text[..caret];
    match typed {
        '*' | '~' if before.ends_with(typed) => {
            let pair = if typed == '*' { "**" } else { "~~" };
            let head = &before[..caret - 1];
            let open = head.rfind(pair)?;
            let content = open + 2..caret - 1;
            tight(&text[content.clone()]).then(|| Closing {
                open: open..open + 2,
                content,
                close: caret - 1..caret,
                mark: if typed == '*' { Mark::Bold } else { Mark::Strike },
            })
        }
        '*' | '_' | '`' => {
            let bytes = before.as_bytes();
            let d = typed as u8;
            let open = (0..caret).rev().find(|&p| {
                bytes[p] == d
                    && (p == 0 || bytes[p - 1] != d)
                    && bytes.get(p + 1) != Some(&d)
                    && (typed != '_' || p == 0 || bytes[p - 1].is_ascii_whitespace())
            })?;
            let content = open + 1..caret;
            let mark = match typed {
                '`' => Mark::Code,
                _ => Mark::Italic,
            };
            tight(&text[content.clone()]).then(|| Closing { open: open..open + 1, content, close: caret..caret, mark })
        }
        ')' => {
            let mid = before.rfind("](")?;
            let open = before[..mid].rfind('[')?;
            let url = &before[mid + 2..];
            (tight(&before[open + 1..mid]) && !url.is_empty() && !url.contains(char::is_whitespace)).then(|| Closing {
                open: open..open + 1,
                content: open + 1..mid,
                close: mid..caret,
                mark: Mark::Link(url.to_owned()),
            })
        }
        _ => None,
    }
}

/// Formats the text that typing `typed` at the caret closes, removing its delimiters, and
/// whether it did (the typed character is then dropped).
pub(super) fn close_format(edit: &mut TextEdit, typed: char) -> bool {
    let (a, z) = edit.range();
    if a != z {
        return false;
    }
    let Some(c) = closing(&edit.text, a, typed) else { return false };
    if !edit.allows_mark(&c.mark) {
        return false;
    }
    edit.set_selection(c.close.start, c.close.end);
    edit.insert("");
    edit.set_selection(c.open.start, c.open.end);
    edit.insert("");
    let shift = c.open.len();
    let (start, end) = (c.content.start - shift, c.content.end - shift);
    edit.set_selection(start, end);
    edit.set_mark(c.mark.clone());
    edit.set_caret(end);
    // What's typed next isn't formatted.
    if edit.has_mark(&c.mark) {
        edit.toggle_mark(c.mark);
    }
    true
}
