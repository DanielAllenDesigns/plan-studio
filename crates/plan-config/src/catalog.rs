//! Command-name sources: the id table, the documented default hotkeys and the
//! flyout groups.

use crate::chord::KeyChord;
use crate::hotkeys::{HotkeyFile, NameSource};
use crate::normalize_newlines;
use crate::toolbar::ToolbarSet;
use std::collections::BTreeMap;

/// Command ids that are not in any `Buttons` table but are named in
/// `docs/daniel-chief-setup.md` (section 3a lists them as removed from, or
/// present in, Daniel's toolbar sets). Kept short on purpose: only ids that
/// are directly evidenced, never guessed from neighbouring numbers.
pub const KNOWN_IDS: &[(&str, &str)] = &[
    ("586", "Structural Member Reporting"),
    ("23912", "Tool Search"),
];

/// One name found in a captured Chief inventory document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocEntry {
    /// The flyout/submenu group (`Door Tools`) when the name was listed under one.
    pub group: Option<String>,
    pub name: String,
    /// Default hotkey; empty when the document gives none.
    pub keys: Vec<KeyChord>,
}

/// A flyout group and its members, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocGroup {
    pub name: String,
    pub members: Vec<DocEntry>,
}

/// Everything known about command names.
#[derive(Debug, Clone, Default)]
pub struct CommandCatalog {
    ids: BTreeMap<String, (String, NameSource)>,
    defaults: Vec<(Vec<KeyChord>, String)>,
    groups: Vec<DocGroup>,
}

fn rank(s: NameSource) -> u8 {
    match s {
        NameSource::Xml => 3,
        NameSource::ToolbarButton => 2,
        NameSource::KnownId => 1,
        _ => 0,
    }
}

impl CommandCatalog {
    pub fn new() -> CommandCatalog {
        CommandCatalog::default()
    }

    /// Adds an id -> name pair unless a stronger source already named the id.
    pub fn insert_id(&mut self, id: &str, name: &str, source: NameSource) {
        let better = self
            .ids
            .get(id)
            .is_none_or(|(_, existing)| rank(source) > rank(*existing));
        if better {
            self.ids.insert(id.to_string(), (name.to_string(), source));
        }
    }

    /// (a) Names already present in a parsed hotkey file.
    pub fn add_xml_names(&mut self, file: &HotkeyFile) {
        for b in &file.bindings {
            if let Some(n) = &b.command_name {
                if b.name_source == NameSource::Xml {
                    self.insert_id(&b.command_id, n, NameSource::Xml);
                }
            }
        }
    }

    /// Names from a toolbar file's `Buttons` table.
    pub fn add_toolbar_set(&mut self, set: &ToolbarSet) {
        for (id, name) in &set.button_names {
            self.insert_id(id, name, NameSource::ToolbarButton);
        }
    }

    /// (c) The hardcoded [`KNOWN_IDS`].
    pub fn add_known_ids(&mut self) {
        for (id, name) in KNOWN_IDS {
            self.insert_id(id, name, NameSource::KnownId);
        }
    }

    /// (b) Reads a captured inventory document ([`parse_hotkey_doc`]); returns
    /// how many default hotkeys it contributed.
    pub fn add_hotkey_doc(&mut self, text: &str) -> usize {
        let mut added = 0;
        for e in parse_hotkey_doc(text) {
            if !e.keys.is_empty() {
                let pair = (e.keys.clone(), e.name.clone());
                if !self.defaults.contains(&pair) {
                    self.defaults.push(pair);
                    added += 1;
                }
            }
            if let Some(g) = &e.group {
                match self.groups.iter_mut().find(|x| &x.name == g) {
                    Some(x) => {
                        if !x.members.iter().any(|m| m.name == e.name) {
                            x.members.push(e);
                        }
                    }
                    None => self.groups.push(DocGroup {
                        name: g.clone(),
                        members: vec![e],
                    }),
                }
            }
        }
        added
    }

    /// Catalog from every source this crate knows: the toolbar sets' name
    /// tables, the known ids and the given inventory documents.
    pub fn from_sources(sets: &[ToolbarSet], docs: &[&str]) -> CommandCatalog {
        let mut c = CommandCatalog::new();
        for s in sets {
            c.add_toolbar_set(s);
        }
        c.add_known_ids();
        for d in docs {
            c.add_hotkey_doc(d);
        }
        c
    }

    pub fn name_for_id(&self, id: &str) -> Option<(&str, NameSource)> {
        self.ids.get(id).map(|(n, s)| (n.as_str(), *s))
    }

    /// Distinct names whose documented default hotkey equals `keys`.
    pub fn default_names_for(&self, keys: &[KeyChord]) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (k, n) in &self.defaults {
            if k.as_slice() == keys && !out.contains(&n.as_str()) {
                out.push(n);
            }
        }
        out
    }

    pub fn default_hotkey_count(&self) -> usize {
        self.defaults.len()
    }

    pub fn id_count(&self) -> usize {
        self.ids.len()
    }

    /// Flyout groups by name (`Door Tools`).
    pub fn group(&self, name: &str) -> Option<&DocGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    pub fn groups(&self) -> &[DocGroup] {
        &self.groups
    }
}

// ---------------------------------------------------------------------------
// Inventory-document reader
// ---------------------------------------------------------------------------

enum Ev {
    Group(String),
    Text(String),
}

fn is_fkey(w: &str) -> bool {
    w.len() >= 2 && w.starts_with('F') && w[1..].chars().all(|c| c.is_ascii_digit())
}

fn starts_with_modifier(w: &str) -> bool {
    w.chars()
        .next()
        .is_some_and(|c| matches!(c, '\u{2303}' | '\u{2325}' | '\u{21E7}' | '\u{2318}'))
}

fn events(par: &str) -> Vec<Ev> {
    let mut evs = Vec::new();
    let mut plain = String::new();
    let mut rest = par;
    while let Some(i) = rest.find("**") {
        plain.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find("**") else {
            plain.push_str(after);
            rest = "";
            break;
        };
        let label = &after[..j];
        let cleaned = label
            .trim()
            .trim_end_matches(':')
            .trim_end_matches('\u{2020}')
            .trim()
            .trim_end_matches(':')
            .trim();
        if cleaned.ends_with("Tools") {
            if !plain.trim().is_empty() {
                evs.push(Ev::Text(std::mem::take(&mut plain)));
            }
            plain.clear();
            evs.push(Ev::Group(cleaned.to_string()));
        } else {
            plain.push_str(label);
        }
        rest = &after[j + 2..];
    }
    plain.push_str(rest);
    if !plain.trim().is_empty() {
        evs.push(Ev::Text(plain));
    }
    evs
}

fn split_segments(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let sentence_end = c == '.' && chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if matches!(c, '\u{b7}' | '|' | ';') || sentence_end {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// Splits a trailing hotkey off a segment.
fn split_hotkey(s: &str) -> (String, Vec<KeyChord>) {
    let s = s.trim();
    // `D, H` in backticks.
    if let Some(body) = s.strip_suffix('`') {
        if let Some(i) = body.rfind('`') {
            let keys: Option<Vec<KeyChord>> = body[i + 1..]
                .split(", ")
                .map(KeyChord::parse_symbols)
                .collect();
            if let Some(keys) = keys {
                return (body[..i].trim().to_string(), keys);
            }
        }
    }
    // (S, L) or (C, P, P): a parenthesised run of single keys.
    if let Some(body) = s.strip_suffix(')') {
        if let Some(i) = body.rfind('(') {
            let parts: Vec<&str> = body[i + 1..].split(", ").collect();
            let single = |p: &&str| {
                p.chars().count() == 1
                    && p.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            };
            if parts.len() >= 2 && parts.iter().all(single) {
                let keys: Vec<KeyChord> = parts.iter().map(|p| KeyChord::plain(p)).collect();
                return (body[..i].trim().to_string(), keys);
            }
        }
    }
    // A Mac-symbol chord or a few bare key names as the last word.
    if let Some((head, last)) = s.rsplit_once(char::is_whitespace) {
        let candidate = starts_with_modifier(last)
            || is_fkey(last)
            || matches!(last, "Space" | "\u{2212}" | "+" | "\u{2326}" | "\u{21E5}");
        if candidate {
            if let Some(k) = KeyChord::parse_symbols(last) {
                return (head.trim().to_string(), vec![k]);
            }
        }
    }
    (s.to_string(), Vec::new())
}

fn clean_name(raw: &str) -> Option<String> {
    let mut s = raw.trim();
    s = s.trim_start_matches(['\u{2713}', '\u{2013}', '-']).trim();
    let mut s = s.to_string();
    // Drop "(off)" and any trailing parenthetical note.
    while let Some(i) = s.find(" (") {
        s.truncate(i);
    }
    let s = s
        .trim_end_matches(['\u{25B8}', '\u{2020}'])
        .trim()
        .trim_end_matches("...")
        .trim_end_matches('\u{2026}')
        .trim();
    let first = s.chars().next()?;
    if !(first.is_uppercase() || first.is_ascii_digit())
        || s.contains(':')
        || s.contains('"')
        || s.contains(')')
        || s.chars().count() > 50
    {
        return None;
    }
    Some(s.to_string())
}

fn process_segment(seg: &str, group: &mut Option<String>) -> Option<DocEntry> {
    let mut s = seg.trim().trim_end_matches('.').trim();
    if s.starts_with('(') {
        let j = s.find(')')?;
        s = s[j + 1..].trim_start_matches(':').trim();
    }
    if s.is_empty() {
        return None;
    }
    let lower = s.to_lowercase();
    let mut name_override = None;
    if lower.starts_with("related") {
        *group = None;
        if lower.starts_with("related:") || lower.starts_with("related commands") {
            return None;
        }
        // "related Make / Edit / Explode CAD Block, CAD Block Management `V`":
        // only the last item carries the hotkey.
        let (head, keys) = split_hotkey(&s["related".len()..]);
        if keys.is_empty() {
            return None;
        }
        let last = head.rsplit(", ").next().unwrap_or(&head).to_string();
        name_override = Some((last, keys));
    }
    let (name, keys) = match name_override {
        Some(x) => x,
        None => split_hotkey(s),
    };
    let name = clean_name(&name)?;
    Some(DocEntry {
        group: group.clone(),
        name,
        keys,
    })
}

/// Reads the "Name <hotkey>" pairs out of Daniel's captured Chief inventory
/// documents (`chief-x18-subtools.md`, `chief-x18-menus.md`). Both use
/// `Name ⌃⌥⌘6`, ``Name `D, H` `` and `Name (S, L)` styles. Names listed under
/// a bold `**... Tools:**` label are reported with that group. Tables, bullet
/// lists, headings and Plan Studio design notes are skipped, which also means
/// `chief-x18-toolbars.md` (all tables, no hotkeys) yields nothing.
pub fn parse_hotkey_doc(text: &str) -> Vec<DocEntry> {
    let text = normalize_newlines(text);
    let mut out = Vec::new();
    for par in text.split("\n\n") {
        let p = par.trim();
        let skip = p.is_empty()
            || p.starts_with(['#', '|', '-', '>'])
            || p.starts_with("* ")
            || p.chars().next().is_some_and(|c| c.is_ascii_digit())
            || [
                "Captured",
                "Plan Studio",
                "Notable",
                "Each submenu",
                "Menu bar",
                "Keep ",
            ]
            .iter()
            .any(|pre| p.starts_with(pre));
        if skip {
            continue;
        }
        let flat = p.replace('\n', " ");
        let mut group: Option<String> = None;
        for ev in events(&flat) {
            match ev {
                Ev::Group(g) => group = Some(g),
                Ev::Text(t) => {
                    for seg in split_segments(&t) {
                        if let Some(e) = process_segment(&seg, &mut group) {
                            out.push(e);
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_all_hotkey_styles() {
        let doc = "**Door Tools:** Hinged Door `D, H` \u{b7} Barn Door \u{2303}\u{2325}\u{2318}P \u{b7} Doorway\n\n\
Send to Layout\u{2026} (S, L) \u{b7} Refresh Display F5 \u{b7} Zoom Out \u{2212} \u{b7} Delete \u{2326}.\n\n\
**Stair Tools:** Draw Stairs \u{21E7}Y. Related: Add Stair Breakline, Auto Stairwell.\n\n\
- skip me `X`\n";
        let e = parse_hotkey_doc(doc);
        let find = |n: &str| e.iter().find(|x| x.name == n).unwrap();
        assert_eq!(find("Hinged Door").keys.len(), 2);
        assert_eq!(find("Hinged Door").group.as_deref(), Some("Door Tools"));
        assert_eq!(find("Barn Door").keys[0].to_string(), "Ctrl+Alt+Cmd+P");
        assert!(find("Doorway").keys.is_empty());
        assert_eq!(find("Send to Layout").keys.len(), 2);
        assert_eq!(find("Refresh Display").keys[0].key, "F5");
        assert_eq!(find("Zoom Out").keys[0].key, "-");
        assert_eq!(find("Delete").keys[0].key, "Del");
        assert_eq!(find("Draw Stairs").group.as_deref(), Some("Stair Tools"));
        assert!(e.iter().all(|x| !x.name.starts_with("Add Stair")));
        assert!(e.iter().all(|x| x.name != "skip me"));
    }

    #[test]
    fn catalog_priority_and_default_lookup() {
        let mut c = CommandCatalog::new();
        c.insert_id("1", "Weak", NameSource::KnownId);
        c.insert_id("1", "Strong", NameSource::ToolbarButton);
        c.insert_id("1", "Weaker", NameSource::KnownId);
        assert_eq!(
            c.name_for_id("1"),
            Some(("Strong", NameSource::ToolbarButton))
        );
        c.add_hotkey_doc("Window \u{21E7}W \u{b7} Wall Niche \u{2303}\u{2325}\u{2318}W");
        let k = KeyChord::parse_file("Shift+W").unwrap();
        assert_eq!(c.default_names_for(&[k]), vec!["Window"]);
    }
}
