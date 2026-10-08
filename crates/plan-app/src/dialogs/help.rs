//! The in-app Help viewer (Help > Launch Help, View Tutorial Guide, View
//! Reference Manual): the manual in `docs/manual/*.md`, embedded in the
//! program at build time (`build.rs` lists the files, `include_str!` embeds
//! them), drawn in an egui window with a chapter tree, headings, tables, code
//! blocks, links between chapters and a search over every chapter.
//!
//! The Markdown reader here covers what the manual uses: `#` headings,
//! paragraphs, nested `-` / `*` / `1.` lists, fenced code blocks, pipe
//! tables, `>` quotes, rules, and inline `code`, `**bold**`, `*italic*` and
//! `[links](target)`.

use eframe::egui::{
    self, text::LayoutJob, Align, Align2, Color32, FontId, RichText, TextFormat, Vec2,
};
use std::cell::RefCell;
use std::sync::OnceLock;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/manual_chapters.rs"));
}

/// The manual's first chapter (the table of contents).
pub const INDEX_FILE: &str = "00-index.md";
/// The chapter Help > View Tutorial Guide opens.
pub const TUTORIAL_FILE: &str = "01-getting-started.md";
/// The hotkey chapter the Customize Hotkeys dialog's Help opens.
pub const HOTKEYS_FILE: &str = "13-hotkeys.md";

// ----- the Markdown model -----

/// A run of text with one style.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    /// The link target (`02-walls.md#walls`, a URL).
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
        slug: String,
    },
    Paragraph(Vec<Span>),
    Item {
        depth: usize,
        marker: String,
        spans: Vec<Span>,
    },
    Code(String),
    Table {
        header: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Quote(Vec<Span>),
    Rule,
}

/// A heading of a chapter, for the tree and for link anchors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingRef {
    pub block: usize,
    pub level: u8,
    pub text: String,
    pub slug: String,
}

#[derive(Clone, Debug)]
pub struct Chapter {
    pub file: &'static str,
    pub title: String,
    pub blocks: Vec<Block>,
    pub headings: Vec<HeadingRef>,
}

/// The plain text of spans.
pub fn plain(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// GitHub-style anchor: lower case, spaces to hyphens, punctuation dropped.
pub fn slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    link: Option<String>,
}

fn push_text(out: &mut Vec<Span>, text: &mut String, st: &Style) {
    if text.is_empty() {
        return;
    }
    out.push(Span {
        text: std::mem::take(text),
        bold: st.bold,
        italic: st.italic,
        code: false,
        link: st.link.clone(),
    });
}

/// Index of the first `needle` in `chars` at or after `from`.
fn find_seq(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| chars[i..i + needle.len()] == *needle)
}

fn parse_inline(src: &str, st: &Style, out: &mut Vec<Span>) {
    let chars: Vec<char> = src.chars().collect();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if chars
                .get(i + 1)
                .is_some_and(|n| "\\`*_[]()#+-.!|<>".contains(*n)) =>
            {
                text.push(chars[i + 1]);
                i += 2;
            }
            '`' => {
                let n = chars[i..].iter().take_while(|&&c| c == '`').count();
                let fence = vec!['`'; n];
                match find_seq(&chars, i + n, &fence) {
                    Some(end) => {
                        push_text(out, &mut text, st);
                        let mut code: String = chars[i + n..end].iter().collect();
                        if n > 1 {
                            code = code.trim().to_string();
                        }
                        out.push(Span {
                            text: code,
                            bold: st.bold,
                            italic: st.italic,
                            code: true,
                            link: st.link.clone(),
                        });
                        i = end + n;
                    }
                    None => {
                        text.push_str(&"`".repeat(n));
                        i += n;
                    }
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => {
                // An image: show its alt text.
                match link_parts(&chars, i + 1) {
                    Some((alt, _, next)) => {
                        push_text(out, &mut text, st);
                        let inner = Style {
                            italic: true,
                            ..st.clone()
                        };
                        parse_inline(&alt, &inner, out);
                        i = next;
                    }
                    None => {
                        text.push('!');
                        i += 1;
                    }
                }
            }
            '[' => match link_parts(&chars, i) {
                Some((label, target, next)) => {
                    push_text(out, &mut text, st);
                    let inner = Style {
                        link: Some(target),
                        ..st.clone()
                    };
                    parse_inline(&label, &inner, out);
                    i = next;
                }
                None => {
                    text.push('[');
                    i += 1;
                }
            },
            '*' if chars.get(i + 1) == Some(&'*') => {
                match find_seq(&chars, i + 2, &['*', '*']).filter(|&e| e > i + 2) {
                    Some(end) => {
                        push_text(out, &mut text, st);
                        let inner: String = chars[i + 2..end].iter().collect();
                        let inner_style = Style {
                            bold: true,
                            ..st.clone()
                        };
                        parse_inline(&inner, &inner_style, out);
                        i = end + 2;
                    }
                    None => {
                        text.push_str("**");
                        i += 2;
                    }
                }
            }
            '*' => {
                let opens = chars.get(i + 1).is_some_and(|n| !n.is_whitespace());
                let end = if opens {
                    (i + 1..chars.len()).find(|&j| {
                        chars[j] == '*'
                            && !chars[j - 1].is_whitespace()
                            && chars.get(j + 1) != Some(&'*')
                            && chars[j - 1] != '*'
                    })
                } else {
                    None
                };
                match end {
                    Some(end) => {
                        push_text(out, &mut text, st);
                        let inner: String = chars[i + 1..end].iter().collect();
                        let inner_style = Style {
                            italic: true,
                            ..st.clone()
                        };
                        parse_inline(&inner, &inner_style, out);
                        i = end + 1;
                    }
                    None => {
                        text.push('*');
                        i += 1;
                    }
                }
            }
            _ => {
                text.push(c);
                i += 1;
            }
        }
    }
    push_text(out, &mut text, st);
}

/// `[label](target)` starting at `chars[start] == '['`: the label, the target
/// and the index after the closing parenthesis.
fn link_parts(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut depth = 0;
    let mut close = None;
    for (j, &c) in chars.iter().enumerate().skip(start) {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
            '`' => {}
            _ => {}
        }
    }
    let close = close?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let mut depth = 0;
    let mut end = None;
    for (j, &c) in chars.iter().enumerate().skip(close + 1) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let label: String = chars[start + 1..close].iter().collect();
    let target: String = chars[close + 2..end].iter().collect();
    Some((label, target.trim().to_string(), end + 1))
}

/// Inline Markdown to spans.
pub fn inline(src: &str) -> Vec<Span> {
    let mut out = Vec::new();
    parse_inline(src, &Style::default(), &mut out);
    // Merge neighbours with the same style.
    let mut merged: Vec<Span> = Vec::with_capacity(out.len());
    for s in out {
        match merged.last_mut() {
            Some(p)
                if !p.code
                    && !s.code
                    && p.bold == s.bold
                    && p.italic == s.italic
                    && p.link == s.link =>
            {
                p.text.push_str(&s.text)
            }
            _ => merged.push(s),
        }
    }
    merged
}

fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    // A pipe inside a code span does not split the cell.
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut prev = ' ';
    for c in t.chars() {
        if c == '`' {
            in_code = !in_code;
        }
        if c == '|' && !in_code && prev != '\\' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
        prev = c;
    }
    cells.push(cur.trim().to_string());
    cells
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.contains('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
}

/// `(indent, marker, rest)` of a list item line.
fn list_item(line: &str) -> Option<(usize, String, &str)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(bullet) {
            return Some((indent, "\u{2022}".to_string(), rest));
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits < 4 && t[digits..].starts_with(". ") {
        return Some((indent, t[..digits + 1].to_string(), &t[digits + 2..]));
    }
    None
}

/// Markdown text to blocks.
pub fn parse(md: &str) -> Vec<Block> {
    let md = md.replace("\r\n", "\n");
    let lines: Vec<&str> = md.lines().collect();
    let mut blocks: Vec<Block> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut i = 0;

    fn flush(para: &mut Vec<String>, blocks: &mut Vec<Block>) {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(inline(&para.join(" "))));
            para.clear();
        }
    }

    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            flush(&mut para, &mut blocks);
            let fence = &t[..3];
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            blocks.push(Block::Code(code.join("\n")));
            continue;
        }
        if t.is_empty() {
            flush(&mut para, &mut blocks);
            i += 1;
            continue;
        }
        if t.starts_with("<!--") {
            flush(&mut para, &mut blocks);
            while i < lines.len() && !lines[i].contains("-->") {
                i += 1;
            }
            i += 1;
            continue;
        }
        if let Some(rest) = t.strip_prefix('#') {
            let level = 1 + rest.chars().take_while(|&c| c == '#').count();
            let body = rest.trim_start_matches('#');
            if level <= 6 && body.starts_with(' ') {
                flush(&mut para, &mut blocks);
                let spans = inline(body.trim());
                let slug = slug(&plain(&spans));
                blocks.push(Block::Heading {
                    level: level as u8,
                    spans,
                    slug,
                });
                i += 1;
                continue;
            }
        }
        if t.starts_with('|') && i + 1 < lines.len() && is_table_separator(lines[i + 1]) {
            flush(&mut para, &mut blocks);
            let header: Vec<Vec<Span>> = split_row(t).iter().map(|c| inline(c)).collect();
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let mut cells: Vec<Vec<Span>> =
                    split_row(lines[i]).iter().map(|c| inline(c)).collect();
                cells.resize(header.len(), Vec::new());
                rows.push(cells);
                i += 1;
            }
            blocks.push(Block::Table { header, rows });
            continue;
        }
        if is_rule(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some(rest) = t.strip_prefix('>') {
            flush(&mut para, &mut blocks);
            let mut quote = vec![rest.trim().to_string()];
            i += 1;
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote.push(lines[i].trim().trim_start_matches('>').trim().to_string());
                i += 1;
            }
            blocks.push(Block::Quote(inline(&quote.join(" "))));
            continue;
        }
        if let Some((indent, marker, rest)) = list_item(line) {
            flush(&mut para, &mut blocks);
            let mut body = rest.trim().to_string();
            i += 1;
            // Continuation lines: indented, not another item, not blank.
            while i < lines.len() {
                let next = lines[i];
                let nt = next.trim();
                if nt.is_empty() || list_item(next).is_some() {
                    break;
                }
                let next_indent = next.len() - next.trim_start().len();
                if next_indent <= indent
                    || nt.starts_with("```")
                    || nt.starts_with('|')
                    || nt.starts_with('#')
                {
                    break;
                }
                body.push(' ');
                body.push_str(nt);
                i += 1;
            }
            blocks.push(Block::Item {
                depth: indent / 2,
                marker,
                spans: inline(&body),
            });
            continue;
        }
        para.push(t.to_string());
        i += 1;
    }
    flush(&mut para, &mut blocks);
    blocks
}

impl Chapter {
    fn from_markdown(file: &'static str, md: &str) -> Chapter {
        let blocks = parse(md);
        let headings: Vec<HeadingRef> = blocks
            .iter()
            .enumerate()
            .filter_map(|(block, b)| match b {
                Block::Heading { level, spans, slug } => Some(HeadingRef {
                    block,
                    level: *level,
                    text: plain(spans),
                    slug: slug.clone(),
                }),
                _ => None,
            })
            .collect();
        let title = headings
            .iter()
            .find(|h| h.level == 1)
            .map_or_else(|| file.to_string(), |h| h.text.clone());
        Chapter {
            file,
            title,
            blocks,
            headings,
        }
    }

    /// The short name for the tree: "Chapter 7: Stairs" becomes "7. Stairs".
    pub fn tree_title(&self) -> String {
        match self.title.strip_prefix("Chapter ") {
            Some(rest) => match rest.split_once(": ") {
                Some((n, name)) => format!("{n}. {name}"),
                None => rest.to_string(),
            },
            None => self.title.clone(),
        }
    }

    fn block_text(&self, i: usize) -> String {
        match &self.blocks[i] {
            Block::Heading { spans, .. } | Block::Paragraph(spans) | Block::Quote(spans) => {
                plain(spans)
            }
            Block::Item { spans, .. } => plain(spans),
            Block::Code(t) => t.clone(),
            Block::Table { header, rows } => header
                .iter()
                .chain(rows.iter().flatten())
                .map(|c| plain(c))
                .collect::<Vec<_>>()
                .join(" | "),
            Block::Rule => String::new(),
        }
    }
}

// ----- the index -----

/// Every chapter of the manual, parsed.
pub struct HelpIndex {
    pub chapters: Vec<Chapter>,
}

/// Where a link leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// A chapter, and the block of the heading the anchor names.
    Chapter {
        chapter: usize,
        block: usize,
    },
    External(String),
    /// Not in the manual (a repository file, an unknown anchor's file).
    Missing(String),
}

/// One search hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub chapter: usize,
    pub block: usize,
    /// The nearest heading above (or the hit itself when it is a heading).
    pub heading: String,
    pub snippet: String,
    pub in_heading: bool,
}

impl HelpIndex {
    pub fn from_sources(sources: &[(&'static str, &str)]) -> HelpIndex {
        HelpIndex {
            chapters: sources
                .iter()
                .map(|(file, md)| Chapter::from_markdown(file, md))
                .collect(),
        }
    }

    pub fn chapter_by_file(&self, file: &str) -> Option<usize> {
        self.chapters.iter().position(|c| c.file == file)
    }

    /// Resolves `target` as written in chapter `from`.
    pub fn resolve(&self, from: usize, target: &str) -> LinkTarget {
        let t = target.trim();
        if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("mailto:") {
            return LinkTarget::External(t.to_string());
        }
        let (file, anchor) = match t.split_once('#') {
            Some((f, a)) => (f, Some(a)),
            None => (t, None),
        };
        let chapter = if file.is_empty() {
            from
        } else {
            let name = file.rsplit('/').next().unwrap_or(file);
            match self.chapter_by_file(name) {
                Some(c) => c,
                None => return LinkTarget::Missing(t.to_string()),
            }
        };
        let block = anchor
            .and_then(|a| {
                let a = a.to_lowercase();
                self.chapters[chapter]
                    .headings
                    .iter()
                    .find(|h| h.slug == a)
                    .map(|h| h.block)
            })
            .unwrap_or(0);
        LinkTarget::Chapter { chapter, block }
    }

    /// Chapters whose text has every word of `query` (case-insensitive),
    /// headings first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let mut heads = Vec::new();
        let mut bodies = Vec::new();
        for (ci, ch) in self.chapters.iter().enumerate() {
            let mut heading = ch.title.clone();
            for bi in 0..ch.blocks.len() {
                let text = ch.block_text(bi);
                let is_heading = matches!(ch.blocks[bi], Block::Heading { .. });
                if is_heading {
                    heading = text.clone();
                }
                let low = text.to_lowercase();
                if !terms.iter().all(|t| low.contains(t.as_str())) {
                    continue;
                }
                let hit = Hit {
                    chapter: ci,
                    block: bi,
                    heading: heading.clone(),
                    snippet: snippet(&text, &low, &terms[0]),
                    in_heading: is_heading,
                };
                if is_heading {
                    heads.push(hit);
                } else {
                    bodies.push(hit);
                }
            }
        }
        heads.extend(bodies);
        heads.truncate(limit);
        heads
    }
}

/// About 120 characters of `text` around the first occurrence of `term`.
fn snippet(text: &str, lower: &str, term: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let term_chars: Vec<char> = term.chars().collect();
    let at = find_seq(&lower_chars, 0, &term_chars).unwrap_or(0);
    let start = at.saturating_sub(50);
    let end = (at + term_chars.len() + 70).min(chars.len());
    let mut s: String = chars[start.min(chars.len())..end].iter().collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{}{}{}",
        if start > 0 { "\u{2026}" } else { "" },
        s,
        if end < chars.len() { "\u{2026}" } else { "" }
    )
}

/// The manual embedded in this build.
pub fn index() -> &'static HelpIndex {
    static INDEX: OnceLock<HelpIndex> = OnceLock::new();
    INDEX.get_or_init(|| HelpIndex::from_sources(embedded::CHAPTERS))
}

// ----- the viewer -----

/// Back/forward entries: chapter and block.
type Place = (usize, usize);

#[derive(Default)]
pub struct HelpViewer {
    open: bool,
    chapter: usize,
    /// Block to scroll to on the next frame.
    scroll_to: Option<usize>,
    query: String,
    history: Vec<Place>,
    at: usize,
    message: String,
}

impl HelpViewer {
    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.open
    }

    #[cfg(test)]
    pub fn chapter(&self) -> usize {
        self.chapter
    }

    #[cfg(test)]
    pub fn query(&self) -> &str {
        &self.query
    }

    #[cfg(test)]
    pub fn set_query(&mut self, q: &str) {
        self.query = q.to_string();
    }

    /// Opens the viewer on `file` (optionally `file#anchor`).
    pub fn open_at(&mut self, target: &str) {
        self.open = true;
        match index().resolve(0, target) {
            LinkTarget::Chapter { chapter, block } => self.go(chapter, block),
            _ => self.go(0, 0),
        }
    }

    /// Moves to a place, recording history.
    pub fn go(&mut self, chapter: usize, block: usize) {
        let idx = index();
        if idx.chapters.is_empty() {
            return;
        }
        let chapter = chapter.min(idx.chapters.len() - 1);
        if self.history.get(self.at) != Some(&(chapter, block)) {
            self.history.truncate(self.at + 1);
            self.history.push((chapter, block));
            self.at = self.history.len() - 1;
        }
        self.chapter = chapter;
        self.scroll_to = Some(block);
        self.query.clear();
        self.message.clear();
    }

    pub fn back(&mut self) -> bool {
        if self.at == 0 || self.history.is_empty() {
            return false;
        }
        self.at -= 1;
        let (c, b) = self.history[self.at];
        self.chapter = c;
        self.scroll_to = Some(b);
        true
    }

    pub fn forward(&mut self) -> bool {
        if self.at + 1 >= self.history.len() {
            return false;
        }
        self.at += 1;
        let (c, b) = self.history[self.at];
        self.chapter = c;
        self.scroll_to = Some(b);
        true
    }

    /// Follows a link clicked in the current chapter.
    pub fn follow(&mut self, target: &str) {
        match index().resolve(self.chapter, target) {
            LinkTarget::Chapter { chapter, block } => self.go(chapter, block),
            LinkTarget::External(url) => {
                self.message = match super::app_info::open_external(&url) {
                    Ok(()) => format!("Opened {url}"),
                    Err(e) => e,
                };
            }
            LinkTarget::Missing(t) => {
                self.message = format!("{t} is not part of the built-in manual");
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Plan Studio Help")
            .id(egui::Id::new("plan_studio_help"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(960.0, 680.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| self.body(ui));
        self.open = open;
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let idx = index();
        if idx.chapters.is_empty() {
            ui.label("The manual was not embedded in this build (docs/manual is missing).");
            return;
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.at > 0, egui::Button::new("Back"))
                .clicked()
            {
                self.back();
            }
            if ui
                .add_enabled(
                    self.at + 1 < self.history.len(),
                    egui::Button::new("Forward"),
                )
                .clicked()
            {
                self.forward();
            }
            if ui.button("Contents").clicked() {
                self.go(0, 0);
            }
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("a command, a dialog, a term")
                    .desired_width(260.0),
            );
            if !self.query.is_empty() && ui.button("Clear").clicked() {
                self.query.clear();
            }
            if !self.message.is_empty() {
                ui.weak(&self.message);
            }
        });
        ui.separator();

        egui::SidePanel::left("help_tree")
            .resizable(true)
            .default_width(230.0)
            .show_inside(ui, |ui| self.tree(ui));
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.query.trim().is_empty() {
                self.page(ui);
            } else {
                self.results(ui);
            }
        });
    }

    fn tree(&mut self, ui: &mut egui::Ui) {
        let idx = index();
        let mut go: Option<(usize, usize)> = None;
        egui::ScrollArea::vertical()
            .id_salt("help_tree_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (ci, ch) in idx.chapters.iter().enumerate() {
                    let here = ci == self.chapter;
                    if ui
                        .selectable_label(here && self.query.trim().is_empty(), ch.tree_title())
                        .clicked()
                    {
                        go = Some((ci, 0));
                    }
                    if here {
                        for h in ch.headings.iter().filter(|h| h.level == 2) {
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                if ui.link(&h.text).clicked() {
                                    go = Some((ci, h.block));
                                }
                            });
                        }
                    }
                }
            });
        if let Some((c, b)) = go {
            self.go(c, b);
        }
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let idx = index();
        let hits = idx.search(&self.query, 200);
        ui.strong(format!(
            "{} results for \"{}\"",
            hits.len(),
            self.query.trim()
        ));
        let mut go: Option<(usize, usize)> = None;
        egui::ScrollArea::vertical()
            .id_salt("help_results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for h in &hits {
                    let ch = &idx.chapters[h.chapter];
                    ui.horizontal_wrapped(|ui| {
                        if ui.link(RichText::new(&h.heading).strong()).clicked() {
                            go = Some((h.chapter, h.block));
                        }
                        ui.weak(ch.tree_title());
                    });
                    if !h.in_heading {
                        ui.label(&h.snippet);
                    }
                    ui.add_space(4.0);
                }
            });
        if let Some((c, b)) = go {
            self.go(c, b);
        }
    }

    fn page(&mut self, ui: &mut egui::Ui) {
        let idx = index();
        let ch = &idx.chapters[self.chapter];
        let mut clicked: Option<String> = None;
        let mut scroll_to = self.scroll_to.take();
        if scroll_to.is_some() {
            ui.ctx().request_repaint();
        }
        egui::ScrollArea::vertical()
            .id_salt(("help_page", self.chapter))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                for (bi, block) in ch.blocks.iter().enumerate() {
                    let resp = show_block(ui, bi, block, &mut clicked);
                    if scroll_to == Some(bi) {
                        resp.scroll_to_me(Some(Align::TOP));
                        scroll_to = None;
                    }
                }
                ui.add_space(40.0);
            });
        if let Some(t) = clicked {
            self.follow(&t);
        }
    }
}

// ----- drawing blocks -----

const LINK_COLOR: Color32 = Color32::from_rgb(0x4F, 0x9D, 0xE8);

fn span_format(ui: &egui::Ui, s: &Span, size: f32, heading: bool) -> TextFormat {
    let visuals = ui.visuals();
    let color = if s.link.is_some() {
        LINK_COLOR
    } else if s.bold || heading {
        visuals.strong_text_color()
    } else {
        visuals.text_color()
    };
    TextFormat {
        font_id: if s.code {
            FontId::monospace(size * 0.92)
        } else {
            FontId::proportional(size)
        },
        color,
        italics: s.italic,
        background: if s.code {
            visuals.code_bg_color
        } else {
            Color32::TRANSPARENT
        },
        underline: if s.link.is_some() {
            egui::Stroke::new(1.0_f32, LINK_COLOR)
        } else {
            egui::Stroke::NONE
        },
        ..Default::default()
    }
}

/// Draws spans wrapped to the available width; links are clickable.
fn show_spans(
    ui: &mut egui::Ui,
    spans: &[Span],
    size: f32,
    heading: bool,
    clicked: &mut Option<String>,
) -> egui::Response {
    if spans.iter().all(|s| s.link.is_none()) {
        let mut job = LayoutJob::default();
        for s in spans {
            job.append(&s.text, 0.0, span_format(ui, s, size, heading));
        }
        return ui.add(egui::Label::new(job).wrap());
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for s in spans {
            let mut rt = RichText::new(&s.text).size(size);
            if s.code {
                rt = rt.monospace().background_color(ui.visuals().code_bg_color);
            }
            if s.italic {
                rt = rt.italics();
            }
            if heading || s.bold {
                rt = rt.strong();
            }
            match &s.link {
                Some(target) => {
                    let r = ui.link(rt.color(LINK_COLOR));
                    if r.clicked() {
                        *clicked = Some(target.clone());
                    }
                    r.on_hover_text(target);
                }
                None => {
                    ui.label(rt);
                }
            }
        }
    })
    .response
}

fn show_block(
    ui: &mut egui::Ui,
    bi: usize,
    block: &Block,
    clicked: &mut Option<String>,
) -> egui::Response {
    match block {
        Block::Heading { level, spans, .. } => {
            let size = match level {
                1 => 26.0,
                2 => 21.0,
                3 => 17.0,
                _ => 15.0,
            };
            ui.add_space(if *level <= 2 { 12.0 } else { 6.0 });
            let r = show_spans(ui, spans, size, true, clicked);
            if *level <= 2 {
                ui.separator();
            }
            r
        }
        Block::Paragraph(spans) => {
            let r = show_spans(ui, spans, 14.0, false, clicked);
            ui.add_space(6.0);
            r
        }
        Block::Item {
            depth,
            marker,
            spans,
        } => {
            ui.horizontal_top(|ui| {
                ui.add_space(8.0 + *depth as f32 * 18.0);
                ui.label(marker);
                ui.vertical(|ui| show_spans(ui, spans, 14.0, false, clicked));
            })
            .response
        }
        Block::Code(text) => {
            let r = egui::Frame::new()
                .fill(ui.visuals().code_bg_color)
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt(("help_code", bi))
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(text).monospace().size(13.0))
                                    .extend(),
                            );
                        });
                })
                .response;
            ui.add_space(6.0);
            r
        }
        Block::Quote(spans) => {
            let r = egui::Frame::new()
                .stroke(egui::Stroke::new(2.0_f32, ui.visuals().weak_text_color()))
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| show_spans(ui, spans, 14.0, false, clicked))
                .response;
            ui.add_space(6.0);
            r
        }
        Block::Rule => ui.separator(),
        Block::Table { header, rows } => {
            let r = show_table(ui, bi, header, rows, clicked);
            ui.add_space(8.0);
            r
        }
    }
}

fn show_table(
    ui: &mut egui::Ui,
    bi: usize,
    header: &[Vec<Span>],
    rows: &[Vec<Vec<Span>>],
    clicked: &mut Option<String>,
) -> egui::Response {
    let n = header.len().max(1);
    // Column widths follow the text length, bounded so no column swallows the rest.
    let weights: Vec<f32> = (0..n)
        .map(|c| {
            let longest = std::iter::once(header.get(c))
                .chain(rows.iter().map(|r| r.get(c)))
                .flatten()
                .map(|cell| plain(cell).chars().count())
                .max()
                .unwrap_or(4);
            (longest as f32).clamp(5.0, 60.0)
        })
        .collect();
    let total: f32 = weights.iter().sum();
    let avail = (ui.available_width() - 12.0 * n as f32).max(200.0);
    egui::Grid::new(("help_table", bi))
        .striped(true)
        .spacing(Vec2::new(12.0, 5.0))
        .show(ui, |ui| {
            for (c, cell) in header.iter().enumerate() {
                let w = avail * weights[c] / total;
                ui.scope(|ui| {
                    ui.set_max_width(w);
                    show_spans(ui, cell, 13.5, true, clicked);
                });
            }
            ui.end_row();
            for row in rows {
                for (c, cell) in row.iter().enumerate() {
                    let w = avail * weights.get(c).copied().unwrap_or(10.0) / total;
                    ui.scope(|ui| {
                        ui.set_max_width(w);
                        show_spans(ui, cell, 13.5, false, clicked);
                    });
                }
                ui.end_row();
            }
        })
        .response
}

// ----- the window as the app uses it -----

thread_local! {
    static VIEWER: RefCell<HelpViewer> = RefCell::new(HelpViewer::default());
}

/// Opens Help on a manual file (`"07-stairs.md"`, or `"07-stairs.md#anchor"`).
pub fn open(target: &str) {
    VIEWER.with(|v| v.borrow_mut().open_at(target));
}

/// Draws the Help window when it is open.
pub fn show(ctx: &egui::Context) {
    VIEWER.with(|v| v.borrow_mut().show(ctx));
}

#[cfg(test)]
pub fn is_open() -> bool {
    VIEWER.with(|v| v.borrow().is_open())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn manual_files() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/manual");
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".md"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn the_index_has_every_chapter_in_order_with_titles() {
        let idx = index();
        let files = manual_files();
        assert!(files.len() >= 18, "{files:?}");
        let embedded: Vec<&str> = idx.chapters.iter().map(|c| c.file).collect();
        assert_eq!(
            embedded,
            files.iter().map(String::as_str).collect::<Vec<_>>()
        );
        for c in &idx.chapters {
            assert!(!c.blocks.is_empty(), "{} is empty", c.file);
            assert!(c.title.len() > 3, "{} has no title", c.file);
            assert!(
                c.headings.iter().any(|h| h.level == 2),
                "{} has no sections",
                c.file
            );
        }
        let stairs = &idx.chapters[idx.chapter_by_file("07-stairs.md").unwrap()];
        assert_eq!(stairs.title, "Chapter 7: Stairs");
        assert_eq!(stairs.tree_title(), "7. Stairs");
        assert_eq!(idx.chapters[0].tree_title(), "Plan Studio Reference Manual");
    }

    #[test]
    fn search_finds_a_known_heading_first() {
        let idx = index();
        let hits = idx.search("Auto Stairwell", 50);
        assert!(!hits.is_empty());
        let first = &hits[0];
        assert!(first.in_heading, "headings come first: {first:?}");
        assert_eq!(first.heading, "7.4 Auto Stairwell");
        assert_eq!(idx.chapters[first.chapter].file, "07-stairs.md");
        // Body text is found too, and every word must match.
        assert!(hits.iter().any(|h| !h.in_heading));
        assert!(idx.search("zzzzqqqq nothing", 10).is_empty());
        assert!(idx.search("   ", 10).is_empty());
        // Case does not matter.
        assert_eq!(idx.search("auto STAIRWELL", 50)[0], *first);
    }

    #[test]
    fn links_resolve_between_chapters_and_to_anchors() {
        let idx = index();
        let toc = idx
            .chapters
            .iter()
            .position(|c| c.file == INDEX_FILE)
            .unwrap();
        match idx.resolve(toc, "07-stairs.md") {
            LinkTarget::Chapter { chapter, block } => {
                assert_eq!(idx.chapters[chapter].file, "07-stairs.md");
                assert_eq!(block, 0);
            }
            other => panic!("{other:?}"),
        }
        match idx.resolve(toc, "07-stairs.md#74-auto-stairwell") {
            LinkTarget::Chapter { chapter, block } => {
                let h = idx.chapters[chapter]
                    .headings
                    .iter()
                    .find(|h| h.block == block)
                    .unwrap();
                assert_eq!(h.text, "7.4 Auto Stairwell");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            idx.resolve(toc, "#conventions"),
            LinkTarget::Chapter { chapter, block } if chapter == toc && block > 0
        ));
        assert_eq!(
            idx.resolve(toc, "https://rustup.rs"),
            LinkTarget::External("https://rustup.rs".into())
        );
        assert!(matches!(
            idx.resolve(toc, "../../README.md"),
            LinkTarget::Missing(_)
        ));
        // Every chapter link of the table of contents lands on a chapter.
        let mut n = 0;
        for b in &idx.chapters[toc].blocks {
            if let Block::Table { rows, .. } = b {
                for cell in rows.iter().flatten().flatten() {
                    if let Some(l) = &cell.link {
                        if l.ends_with(".md") {
                            assert!(
                                matches!(idx.resolve(toc, l), LinkTarget::Chapter { .. }),
                                "{l}"
                            );
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n >= 17, "{n} chapter links in the contents table");
    }

    #[test]
    fn markdown_blocks_and_inline_styles_parse() {
        let md = "# Title\n\nSome **bold**, *italic*, `code` and [a link](x.md#y) here.\n\n\
                  - one\n  - nested\n- two\n  continued\n\n1. first\n2. second\n\n\
                  ```\nlet x = 1;\n```\n\n| A | B |\n|---|---|\n| 1 | `a|b` |\n\n> quoted\n> text\n\n---\n";
        let b = parse(md);
        assert!(matches!(&b[0], Block::Heading { level: 1, slug, .. } if slug == "title"));
        let Block::Paragraph(p) = &b[1] else {
            panic!("{:?}", b[1])
        };
        assert!(p.iter().any(|s| s.bold && s.text == "bold"));
        assert!(p.iter().any(|s| s.italic && s.text == "italic"));
        assert!(p.iter().any(|s| s.code && s.text == "code"));
        assert!(p
            .iter()
            .any(|s| s.link.as_deref() == Some("x.md#y") && s.text == "a link"));
        assert!(matches!(&b[2], Block::Item { depth: 0, marker, .. } if marker == "\u{2022}"));
        assert!(matches!(&b[3], Block::Item { depth: 1, .. }));
        let Block::Item { spans, .. } = &b[4] else {
            panic!()
        };
        assert_eq!(plain(spans), "two continued");
        assert!(matches!(&b[5], Block::Item { marker, .. } if marker == "1."));
        assert!(matches!(&b[7], Block::Code(c) if c == "let x = 1;"));
        let Block::Table { header, rows } = &b[8] else {
            panic!("{:?}", b[8])
        };
        assert_eq!(header.len(), 2);
        assert_eq!(
            plain(&rows[0][1]),
            "a|b",
            "a pipe in code stays in the cell"
        );
        assert!(matches!(&b[9], Block::Quote(q) if plain(q) == "quoted text"));
        assert!(matches!(b[10], Block::Rule));
        assert_eq!(slug("7.4 Auto Stairwell"), "74-auto-stairwell");
        assert_eq!(
            slug("Dialogs: Staircase & Landing"),
            "dialogs-staircase--landing"
        );
    }

    #[test]
    fn every_chapter_parses_without_leaving_markup_behind() {
        for c in &index().chapters {
            for b in &c.blocks {
                let text = match b {
                    Block::Heading { spans, .. }
                    | Block::Paragraph(spans)
                    | Block::Quote(spans)
                    | Block::Item { spans, .. } => plain(spans),
                    _ => continue,
                };
                assert!(
                    !text.contains("**"),
                    "{}: unparsed bold in {text:?}",
                    c.file
                );
            }
        }
    }

    #[test]
    fn the_viewer_navigates_with_history_and_follows_links() {
        let mut v = HelpViewer::default();
        v.open_at("07-stairs.md");
        assert!(v.is_open());
        assert_eq!(index().chapters[v.chapter()].file, "07-stairs.md");
        v.follow("08-roofs.md");
        assert_eq!(index().chapters[v.chapter()].file, "08-roofs.md");
        assert!(v.back());
        assert_eq!(index().chapters[v.chapter()].file, "07-stairs.md");
        assert!(v.forward());
        assert_eq!(index().chapters[v.chapter()].file, "08-roofs.md");
        assert!(!v.forward());
        v.set_query("stairs");
        v.go(0, 0);
        assert!(v.query().is_empty(), "navigating ends the search");
        v.follow("../../README.md");
        assert!(v.message.contains("not part of"));
    }

    #[test]
    fn the_window_draws_headlessly_on_every_chapter() {
        let ctx = egui::Context::default();
        let mut v = HelpViewer::default();
        for file in [
            "00-index.md",
            "07-stairs.md",
            "13-hotkeys.md",
            "15-glossary.md",
        ] {
            v.open_at(file);
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| v.show(ctx));
            }
        }
        v.set_query("hotkey");
        let _ = ctx.run(egui::RawInput::default(), |ctx| v.show(ctx));
        assert!(v.is_open());
    }
}
