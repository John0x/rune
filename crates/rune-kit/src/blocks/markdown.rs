//! Blocks to Markdown and back: CommonMark with GFM task lists and strikethrough. Nesting
//! survives under list items (four spaces a level); a block nested under anything else is
//! written at the top level. A toggle is a list item starting `▸ ` (closed) or `▾ ` (open);
//! underline and colors are inline HTML, colors as Notion's HTML export writes them
//! (`<mark class="highlight-red">`, `<mark class="highlight-red_background">`).

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rune::{Mark, MarkRange};

use super::{Block, COLORS, Kind};

/// The markers that start a toggle's text: closed, open.
const TOGGLE: [&str; 2] = ["▸ ", "▾ "];

/// A parsed block: kind, nesting, text and marks.
pub(super) type Part = (Kind, u8, String, Vec<MarkRange>);

#[derive(Clone, Copy, PartialEq)]
enum Container {
    Quote,
    List {
        ordered: bool,
    },
    /// A list item, and whether its own block was taken by its first paragraph.
    Item {
        used: bool,
    },
}

#[derive(Default)]
struct Reader {
    parts: Vec<Part>,
    containers: Vec<Container>,
    /// The block being read, and whether it's a list item's own.
    current: Option<(Part, bool)>,
    open: Vec<(Mark, usize)>,
}

impl Reader {
    fn depth(&self) -> u8 {
        self.containers.iter().filter(|c| matches!(c, Container::List { .. })).count() as u8
    }

    fn flush(&mut self) {
        if let Some((mut part, _)) = self.current.take() {
            if part.0 == Kind::Bullet
                && let Some(open) = TOGGLE.iter().position(|m| part.2.starts_with(m))
            {
                let n = TOGGLE[open].len();
                part.0 = Kind::Toggle { open: open == 1 };
                part.2.drain(..n);
                part.3.retain_mut(|m| {
                    (m.start, m.end) = (m.start.saturating_sub(n), m.end.saturating_sub(n));
                    m.start < m.end
                });
            }
            self.parts.push(part);
        }
        self.open.clear();
    }

    /// Inline HTML: underline and Notion's colors open and close; anything else is text.
    fn html(&mut self, tag: &str) {
        let tag = tag.trim();
        let mark = match tag {
            "<u>" => Some(Mark::Underline),
            _ => tag.strip_prefix("<mark class=\"highlight-").and_then(|rest| rest.strip_suffix("\">")).and_then(
                |name| {
                    let (name, background) = name.strip_suffix("_background").map_or((name, false), |n| (n, true));
                    let n = COLORS.iter().position(|c| c.eq_ignore_ascii_case(name))? as u8;
                    Some(if background { Mark::Highlight(n) } else { Mark::Color(n) })
                },
            ),
        };
        if let Some(mark) = mark {
            self.open.push((mark, self.len()));
            return;
        }
        let closes = |m: &Mark| match tag {
            "</u>" => *m == Mark::Underline,
            "</mark>" => matches!(m, Mark::Color(_) | Mark::Highlight(_)),
            _ => false,
        };
        match self.open.iter().rposition(|(m, _)| closes(m)) {
            Some(at) => {
                let (mark, start) = self.open.remove(at);
                self.mark(mark, start);
            }
            None => self.text(tag),
        }
    }

    fn start(&mut self, kind: Kind, indent: u8) {
        self.flush();
        self.current = Some(((kind, indent, String::new(), Vec::new()), false));
    }

    /// A paragraph in the current context: a list item's text, a quote, or a plain one.
    fn start_paragraph(&mut self) {
        if let Some(Container::Item { used }) = self.containers.last_mut()
            && !*used
            && self.current.as_ref().is_some_and(|(_, own)| *own)
        {
            *used = true;
            return;
        }
        let depth = self.depth();
        let kind = if self.containers.last() == Some(&Container::Quote) { Kind::Quote } else { Kind::Paragraph };
        self.start(kind, depth);
    }

    fn text(&mut self, s: &str) {
        if self.current.is_none() {
            self.start_paragraph();
        }
        if let Some(((_, _, text, _), _)) = &mut self.current {
            text.push_str(s);
        }
    }

    fn len(&self) -> usize {
        self.current.as_ref().map_or(0, |((_, _, text, _), _)| text.len())
    }

    fn mark(&mut self, mark: Mark, start: usize) {
        let end = self.len();
        if let Some(((_, _, _, marks), _)) = &mut self.current
            && end > start
        {
            marks.push(MarkRange::new(start..end, mark));
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.open_tag(tag),
            Event::End(tag) => self.close_tag(tag),
            Event::Text(t) | Event::Html(t) => self.text(&t),
            Event::InlineHtml(t) => self.html(&t),
            Event::Code(t) => {
                let start = self.len();
                self.text(&t);
                self.mark(Mark::Code, start);
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.text("\n"),
            Event::Rule => {
                let depth = self.depth();
                self.start(Kind::Divider, depth);
                self.flush();
            }
            Event::TaskListMarker(checked) => {
                if let Some(((kind, ..), _)) = &mut self.current {
                    *kind = Kind::Todo { checked };
                }
            }
            _ => {}
        }
    }

    fn open_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.start_paragraph(),
            Tag::Heading { level, .. } => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    _ => 3,
                };
                let depth = self.depth();
                self.start(Kind::Heading(level), depth);
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                let depth = self.depth();
                self.start(Kind::Code { language }, depth);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.containers.push(Container::Quote);
            }
            Tag::List(start) => {
                self.flush();
                self.containers.push(Container::List { ordered: start.is_some() });
            }
            Tag::Item => {
                let ordered = self.containers.iter().rev().find_map(|c| match c {
                    Container::List { ordered } => Some(*ordered),
                    _ => None,
                });
                let depth = self.depth().saturating_sub(1);
                self.start(if ordered == Some(true) { Kind::Numbered } else { Kind::Bullet }, depth);
                if let Some((_, own)) = &mut self.current {
                    *own = true;
                }
                self.containers.push(Container::Item { used: false });
            }
            Tag::TableHead | Tag::TableRow => self.start(Kind::Paragraph, 0),
            Tag::Strong => self.open.push((Mark::Bold, self.len())),
            Tag::Emphasis => self.open.push((Mark::Italic, self.len())),
            Tag::Strikethrough => self.open.push((Mark::Strike, self.len())),
            Tag::Link { dest_url, .. } => self.open.push((Mark::Link(dest_url.into_string()), self.len())),
            _ => {}
        }
    }

    fn close_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::TableHead | TagEnd::TableRow => {
                self.flush()
            }
            TagEnd::CodeBlock => {
                if let Some(((_, _, text, _), _)) = &mut self.current
                    && text.ends_with('\n')
                {
                    text.pop();
                }
                self.flush();
            }
            TagEnd::TableCell => self.text(" | "),
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                if let Some((mark, start)) = self.open.pop() {
                    self.mark(mark, start);
                }
            }
            _ => {}
        }
        if matches!(tag, TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item) {
            self.flush();
            self.containers.pop();
        }
    }
}

/// The blocks `markdown` describes.
pub(super) fn parse(markdown: &str) -> Vec<Part> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    let mut reader = Reader::default();
    for event in Parser::new_ext(markdown, options) {
        reader.event(event);
    }
    reader.flush();
    reader.parts
}

/// `blocks` as Markdown.
pub(super) fn write(blocks: &[Block]) -> String {
    let levels = levels(blocks);
    let mut out = String::new();
    let mut numbers: Vec<usize> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        let level = levels[i];
        numbers.truncate(level as usize + 1);
        numbers.resize(level as usize + 1, 0);
        if b.kind == Kind::Numbered {
            numbers[level as usize] += 1;
        } else {
            numbers[level as usize] = 0;
        }
        if i > 0 {
            let tight = blocks[i - 1].kind.is_list() && b.kind.is_list();
            out.push_str(if tight { "\n" } else { "\n\n" });
        }
        let pad = "    ".repeat(level as usize);
        write_block(&mut out, b, &pad, numbers[level as usize]);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Each block's nesting in Markdown: one deeper than its parent when that's a list item.
fn levels(blocks: &[Block]) -> Vec<u8> {
    let mut levels = vec![0u8; blocks.len()];
    for i in 0..blocks.len() {
        let parent = blocks[..i].iter().rposition(|p| p.indent < blocks[i].indent);
        levels[i] = match parent {
            Some(p) if blocks[p].kind.is_list() => levels[p] + 1,
            _ => 0,
        };
    }
    levels
}

fn write_block(out: &mut String, b: &Block, pad: &str, number: usize) {
    let text = &b.edit.text;
    let (marker, rest) = match &b.kind {
        Kind::Divider => {
            out.push_str(pad);
            out.push_str("---");
            return;
        }
        Kind::Code { language } => {
            let fence = "`".repeat(longest_run(text, '`').max(2) + 1);
            out.push_str(&format!("{pad}{fence}{language}\n"));
            for line in text.split('\n') {
                out.push_str(pad);
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(pad);
            out.push_str(&fence);
            return;
        }
        Kind::Paragraph => (String::new(), pad.to_owned()),
        Kind::Heading(level) => (format!("{} ", "#".repeat(*level as usize)), String::new()),
        Kind::Bullet => ("- ".to_owned(), format!("{pad}  ")),
        Kind::Numbered => (format!("{number}. "), format!("{pad}{}", " ".repeat(number.to_string().len() + 2))),
        Kind::Todo { checked } => ((if *checked { "- [x] " } else { "- [ ] " }).to_owned(), format!("{pad}  ")),
        Kind::Toggle { open } => (format!("- {}", TOGGLE[usize::from(*open)]), format!("{pad}  ")),
        Kind::Quote => ("> ".to_owned(), format!("{pad}> ")),
    };
    out.push_str(pad);
    out.push_str(&marker);
    let body = inline(text, b.edit.marks());
    let body = if marker.is_empty() || b.kind == Kind::Quote { escape_start(&body) } else { body };
    let joiner = if matches!(b.kind, Kind::Heading(_)) { " ".to_owned() } else { format!("\\\n{rest}") };
    out.push_str(&body.replace('\n', &joiner));
}

fn longest_run(text: &str, c: char) -> usize {
    text.split(|x| x != c).map(str::len).max().unwrap_or(0)
}

/// Escapes what would start a heading, list or quote at the start of a paragraph.
fn escape_start(body: &str) -> String {
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    let list = body.starts_with("- ") || body.starts_with("+ ") || body.starts_with("* ");
    let numbered = digits > 0 && body[digits..].starts_with(". ");
    if body.starts_with('#') || body.starts_with('>') || list {
        format!("\\{body}")
    } else if numbered {
        format!("{}\\{}", &body[..digits], &body[digits..])
    } else {
        body.to_owned()
    }
}

/// The order marks nest in, outermost first; [`UNWRITTEN`] for those Markdown drops.
fn rank(mark: &Mark) -> u8 {
    match mark {
        Mark::Link(_) => 0,
        Mark::Bold => 1,
        Mark::Italic => 2,
        Mark::Color(_) => 3,
        Mark::Highlight(_) => 4,
        Mark::Underline => 5,
        Mark::Strike => 6,
        Mark::Code => 7,
        _ => UNWRITTEN,
    }
}

const UNWRITTEN: u8 = 8;

fn opener(mark: &Mark, code: &str) -> String {
    let color = |n: &u8| COLORS.get(usize::from(*n)).copied().unwrap_or("gray").to_lowercase();
    match mark {
        Mark::Bold => "**".into(),
        Mark::Italic => "*".into(),
        Mark::Underline => "<u>".into(),
        Mark::Strike => "~~".into(),
        Mark::Link(_) => "[".into(),
        Mark::Code => code.into(),
        Mark::Color(n) => format!("<mark class=\"highlight-{}\">", color(n)),
        Mark::Highlight(n) => format!("<mark class=\"highlight-{}_background\">", color(n)),
        _ => String::new(),
    }
}

fn closer(mark: &Mark, code: &str) -> String {
    match mark {
        Mark::Link(url) => format!("]({url})"),
        Mark::Underline => "</u>".into(),
        Mark::Color(_) | Mark::Highlight(_) => "</mark>".into(),
        mark => opener(mark, code),
    }
}

/// `text` with its marks as Markdown delimiters. Spaces at a run's edges go outside its
/// delimiters, which Markdown requires.
fn inline(text: &str, marks: &[MarkRange]) -> String {
    let mut cuts: Vec<usize> = vec![0, text.len()];
    for m in marks {
        cuts.extend([m.start, m.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let code = if marks.iter().any(|m| m.mark == Mark::Code && text[m.range()].contains('`')) { "``" } else { "`" };
    let mut out = String::new();
    let mut stack: Vec<Mark> = Vec::new();
    let mut held = String::new();
    for w in cuts.windows(2) {
        let (a, z) = (w[0], w[1]);
        let segment = &text[a..z];
        let mut want: Vec<Mark> = marks
            .iter()
            .filter(|m| m.start <= a && z <= m.end && rank(&m.mark) < UNWRITTEN)
            .map(|m| m.mark.clone())
            .collect();
        want.sort_by_key(rank);
        want.dedup();
        let core = segment.trim();
        if core.is_empty() {
            want.retain(|m| stack.contains(m));
        }
        while !stack.iter().all(|m| want.contains(m)) {
            let top = stack.pop().unwrap();
            out.push_str(&closer(&top, code));
        }
        out.push_str(&std::mem::take(&mut held));
        let opening: Vec<Mark> = want.iter().filter(|m| !stack.contains(m)).cloned().collect();
        let mut body = segment;
        if !opening.is_empty() {
            let start = segment.len() - segment.trim_start().len();
            out.push_str(&segment[..start]);
            body = &segment[start..];
        }
        for m in opening {
            out.push_str(&opener(&m, code));
            stack.push(m);
        }
        // Trailing spaces wait until the next run shows whether the marks close before them.
        let kept = if stack.is_empty() { body } else { body.trim_end() };
        held = body[kept.len()..].to_owned();
        out.push_str(&if stack.contains(&Mark::Code) { kept.to_owned() } else { escape(kept) });
    }
    while let Some(top) = stack.pop() {
        out.push_str(&closer(&top, code));
    }
    out.push_str(&held);
    out
}

/// Escapes the characters Markdown would read as formatting.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let edge = |j: Option<&char>| j.is_none_or(|c| c.is_whitespace() || c.is_ascii_punctuation());
        let escaped = match c {
            '\\' | '*' | '`' | '[' | ']' => true,
            '<' => chars.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == '/'),
            '_' => edge(i.checked_sub(1).and_then(|j| chars.get(j))) || edge(chars.get(i + 1)),
            '~' => chars.get(i + 1) == Some(&'~') || (i > 0 && chars[i - 1] == '~'),
            _ => false,
        };
        if escaped {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
