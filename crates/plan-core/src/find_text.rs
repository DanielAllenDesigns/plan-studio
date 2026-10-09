//! Edit > Find/Replace Text (TXT-12, TXT-45, TXT-46; manual pp. 543, 544):
//! finds and replaces text in the plan's text objects: Text and Rich Text,
//! and the labels of callouts, markers and notes, including their rich-text
//! runs and the macros they hold.
//!
//! # What is searched
//!
//! The text as it was typed. A live text (one with `%macros%`, see
//! [`crate::macros`]) is searched by its source, so the *names* of the macros
//! are found, not their values. [`MacroMode`] says whether those macro names
//! take part: Exclude Macros ignores them, Macros Only finds nothing else,
//! Include All finds both. With Expand Percent Signs a run of one or two `%`
//! outside a macro is ignored while matching (imported text uses them to mark
//! style commands).
//!
//! The generated text of a callout, marker or note is not searched; its
//! record is (the sync pass writes the text again).
//!
//! # Scopes
//!
//! [`Scope::CurrentView`] is one floor, [`Scope::CurrentFile`] and
//! [`Scope::AllOpenFiles`] every floor of the plan (the app adds the layout
//! files of the plan to the second), [`Scope::SelectedObjects`] the text
//! objects and annotations in a selection.

use crate::cad::CadItem;
use crate::macros::tokens;
use crate::model::Project;
use crate::text_styles::{runs_plain, RichRun};
use crate::Id;
use std::collections::HashSet;

/// Whether the names of macros take part in the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MacroMode {
    /// Macros and ordinary text.
    #[default]
    IncludeAll,
    ExcludeMacros,
    MacrosOnly,
}

impl MacroMode {
    pub const ALL: [MacroMode; 3] = [
        MacroMode::ExcludeMacros,
        MacroMode::MacrosOnly,
        MacroMode::IncludeAll,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MacroMode::IncludeAll => "Include All",
            MacroMode::ExcludeMacros => "Exclude Macros",
            MacroMode::MacrosOnly => "Macros Only",
        }
    }
}

/// Searching In.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    CurrentView,
    CurrentFile,
    AllOpenFiles,
    SelectedObjects,
}

impl Scope {
    pub const ALL: [Scope; 4] = [
        Scope::CurrentView,
        Scope::CurrentFile,
        Scope::AllOpenFiles,
        Scope::SelectedObjects,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scope::CurrentView => "Current View",
            Scope::CurrentFile => "Current File",
            Scope::AllOpenFiles => "All Open Files",
            Scope::SelectedObjects => "Selected Objects",
        }
    }
}

/// The part of the plan a search looks in.
#[derive(Debug, Clone, Copy)]
pub struct Area<'a> {
    pub scope: Scope,
    /// The floor of the current view.
    pub floor: usize,
    /// The CAD objects of the selection (Selected Objects).
    pub selected: &'a [Id],
}

impl<'a> Area<'a> {
    pub fn new(scope: Scope, floor: usize, selected: &'a [Id]) -> Self {
        Self {
            scope,
            floor,
            selected,
        }
    }
}

/// What to look for and what replaces it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextSearch {
    pub find: String,
    pub replace: String,
    /// Case Sensitive.
    pub match_case: bool,
    /// Only whole words match (neighbours are not letters, digits or `_`).
    pub whole_word: bool,
    /// Expand Percent Signs: one or two `%` in a row are ignored.
    pub expand_percent: bool,
    pub macros: MacroMode,
}

/// A text object that contains the search text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMatch {
    pub floor: usize,
    pub id: Id,
    pub text: String,
    /// How many times the search text occurs in it.
    pub count: usize,
}

/// Which text of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// A Text or Rich Text object.
    Text,
    CalloutLabel,
    CalloutBelow,
    MarkerLabel,
    MarkerBelow,
    NoteLabel,
    NoteBelow,
    NoteText,
}

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::Text => "Text",
            Field::CalloutLabel => "Callout label",
            Field::CalloutBelow => "Callout text below line",
            Field::MarkerLabel => "Marker label",
            Field::MarkerBelow => "Marker text below line",
            Field::NoteLabel => "Note label",
            Field::NoteBelow => "Note text below line",
            Field::NoteText => "Note schedule text",
        }
    }
}

/// One text of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    pub floor: usize,
    pub field: Field,
    /// The CAD object of a text; the record index of an annotation.
    pub index: usize,
    /// The CAD object that selects it (a text's own, an annotation's first).
    pub id: Id,
}

/// One occurrence of the search text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextHit {
    pub site: Site,
    /// First character of the occurrence in [`TextHit::text`].
    pub start: usize,
    /// Length in characters.
    pub len: usize,
    /// The text searched (the source of a live text).
    pub text: String,
    /// The occurrence lies inside a `%macro%`.
    pub in_macro: bool,
}

// ===== matching =====

fn same_char(a: char, b: char, match_case: bool) -> bool {
    a == b || (!match_case && a.to_lowercase().eq(b.to_lowercase()))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `(start, len)` in characters of the matches of `q` in `text`, left to
/// right and not overlapping, honouring Expand Percent Signs and the Macro
/// Options. The lengths are in the original text (an ignored `%` inside a
/// match counts).
pub fn occurrences(text: &str, q: &TextSearch) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = q.find.chars().collect();
    if needle.is_empty() || chars.is_empty() {
        return Vec::new();
    }
    // Which characters belong to a `%macro%`.
    let mut in_macro = vec![false; chars.len()];
    if text.contains('%') {
        let byte_to_char: Vec<usize> = {
            let mut v = vec![0; text.len() + 1];
            for (ci, (bi, c)) in text.char_indices().enumerate() {
                for k in 0..c.len_utf8() {
                    v[bi + k] = ci;
                }
            }
            v[text.len()] = chars.len();
            v
        };
        for (s, e, _) in tokens(text) {
            for slot in in_macro
                .iter_mut()
                .take(byte_to_char[e])
                .skip(byte_to_char[s])
            {
                *slot = true;
            }
        }
    }
    // Characters ignored while matching (Expand Percent Signs).
    let mut skip = vec![false; chars.len()];
    if q.expand_percent {
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '%' {
                let mut j = i;
                while j < chars.len() && chars[j] == '%' {
                    j += 1;
                }
                if j - i <= 2 {
                    for k in i..j {
                        if !in_macro[k] {
                            skip[k] = true;
                        }
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    let kept: Vec<usize> = (0..chars.len()).filter(|i| !skip[*i]).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= kept.len() {
        let hit = needle
            .iter()
            .enumerate()
            .all(|(k, n)| same_char(chars[kept[i + k]], *n, q.match_case));
        let first = kept[i];
        let last = kept[i + needle.len() - 1];
        let bounded = !q.whole_word
            || ((i == 0 || !is_word(chars[kept[i - 1]]))
                && (i + needle.len() == kept.len() || !is_word(chars[kept[i + needle.len()]])));
        let span = first..=last;
        let macro_ok = match q.macros {
            MacroMode::IncludeAll => true,
            MacroMode::ExcludeMacros => span.clone().all(|k| !in_macro[k]),
            MacroMode::MacrosOnly => span.clone().all(|k| in_macro[k] || skip[k]),
        };
        if hit && bounded && macro_ok {
            out.push((first, last - first + 1));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// How many times `q.find` occurs in `text`.
pub fn count_matches(text: &str, q: &TextSearch) -> usize {
    occurrences(text, q).len()
}

/// `text` with the character ranges `ranges` (sorted, not overlapping)
/// replaced by `with`.
pub fn replace_ranges(text: &str, ranges: &[(usize, usize)], with: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for &(s, l) in ranges {
        out.extend(&chars[at..s.min(chars.len())]);
        out.push_str(with);
        at = (s + l).min(chars.len());
    }
    out.extend(&chars[at..]);
    out
}

/// `text` with every match of `q.find` replaced by `q.replace`, and how many
/// were replaced.
pub fn replace_in(text: &str, q: &TextSearch) -> (String, usize) {
    let found = occurrences(text, q);
    if found.is_empty() {
        return (text.to_string(), 0);
    }
    (replace_ranges(text, &found, &q.replace), found.len())
}

/// The runs after replacing `ranges` (character ranges of the joined text)
/// by `with`: each run keeps its format when every range lies inside one
/// run; otherwise one run with the first run's format holds the result.
pub fn replace_in_runs(runs: &[RichRun], ranges: &[(usize, usize)], with: &str) -> Vec<RichRun> {
    if runs.is_empty() {
        return Vec::new();
    }
    let mut starts = Vec::with_capacity(runs.len());
    let mut at = 0;
    for r in runs {
        starts.push(at);
        at += r.text.chars().count();
    }
    let inside = |s: usize, l: usize| {
        runs.iter().enumerate().any(|(i, r)| {
            let (a, b) = (starts[i], starts[i] + r.text.chars().count());
            s >= a && s + l <= b
        })
    };
    if ranges.iter().all(|&(s, l)| inside(s, l)) {
        runs.iter()
            .enumerate()
            .map(|(i, r)| {
                let (a, n) = (starts[i], r.text.chars().count());
                let local: Vec<(usize, usize)> = ranges
                    .iter()
                    .filter(|&&(s, l)| s >= a && s + l <= a + n)
                    .map(|&(s, l)| (s - a, l))
                    .collect();
                RichRun {
                    text: replace_ranges(&r.text, &local, with),
                    ..r.clone()
                }
            })
            .collect()
    } else {
        let joined = runs_plain(runs);
        vec![RichRun {
            text: replace_ranges(&joined, ranges, with),
            ..runs[0].clone()
        }]
    }
}

// ===== the plan =====

impl Project {
    fn floors_in(&self, area: &Area) -> std::ops::Range<usize> {
        match area.scope {
            Scope::CurrentView => area.floor..(area.floor + 1).min(self.floors.len()),
            _ => 0..self.floors.len(),
        }
    }

    /// The texts of the plan in `area`: floor by floor, the text objects in
    /// drawing order, then the callouts, markers and notes.
    pub fn text_sites(&self, area: &Area) -> Vec<Site> {
        let selected: HashSet<Id> = area.selected.iter().copied().collect();
        let only_selected = area.scope == Scope::SelectedObjects;
        let mut out = Vec::new();
        for fi in self.floors_in(area) {
            let f = &self.floors[fi];
            let generated: HashSet<Id> = f
                .annots
                .callouts
                .iter()
                .flat_map(|c| c.items.iter())
                .chain(f.annots.markers.iter().flat_map(|c| c.items.iter()))
                .chain(f.annots.notes.iter().flat_map(|c| c.items.iter()))
                .copied()
                .collect();
            for o in &f.cad {
                if matches!(o.item, CadItem::Text { .. })
                    && !generated.contains(&o.id)
                    && (!only_selected || selected.contains(&o.id))
                {
                    out.push(Site {
                        floor: fi,
                        field: Field::Text,
                        index: 0,
                        id: o.id,
                    });
                }
            }
            let picked =
                |items: &[Id]| !only_selected || items.iter().any(|i| selected.contains(i));
            let first = |items: &[Id]| items.first().copied().unwrap_or(0);
            for (i, c) in f.annots.callouts.iter().enumerate() {
                if picked(&c.items) {
                    for field in [Field::CalloutLabel, Field::CalloutBelow] {
                        out.push(Site {
                            floor: fi,
                            field,
                            index: i,
                            id: first(&c.items),
                        });
                    }
                }
            }
            for (i, m) in f.annots.markers.iter().enumerate() {
                if picked(&m.items) {
                    for field in [Field::MarkerLabel, Field::MarkerBelow] {
                        out.push(Site {
                            floor: fi,
                            field,
                            index: i,
                            id: first(&m.items),
                        });
                    }
                }
            }
            for (i, n) in f.annots.notes.iter().enumerate() {
                if picked(&n.items) {
                    for field in [Field::NoteLabel, Field::NoteBelow, Field::NoteText] {
                        out.push(Site {
                            floor: fi,
                            field,
                            index: i,
                            id: first(&n.items),
                        });
                    }
                }
            }
        }
        out
    }

    /// The text of a site as it was typed.
    pub fn site_text(&self, s: &Site) -> Option<String> {
        let f = self.floors.get(s.floor)?;
        Some(match s.field {
            Field::Text => {
                if let Some(live) = self.live_text(s.id) {
                    live.source()
                } else {
                    match &f.cad.iter().find(|o| o.id == s.id)?.item {
                        CadItem::Text { text, .. } => text.clone(),
                        _ => return None,
                    }
                }
            }
            Field::CalloutLabel => f.annots.callouts.get(s.index)?.label.clone(),
            Field::CalloutBelow => f.annots.callouts.get(s.index)?.text_below.clone(),
            Field::MarkerLabel => f.annots.markers.get(s.index)?.label.text.clone(),
            Field::MarkerBelow => f.annots.markers.get(s.index)?.below.text.clone(),
            Field::NoteLabel => f.annots.notes.get(s.index)?.label.clone(),
            Field::NoteBelow => f.annots.notes.get(s.index)?.text_below.clone(),
            Field::NoteText => f.annots.notes.get(s.index)?.text.clone(),
        })
    }

    /// Every occurrence of `q.find` in `area`, in reading order.
    pub fn find_hits(&self, q: &TextSearch, area: &Area) -> Vec<TextHit> {
        let mut out = Vec::new();
        if q.find.is_empty() {
            return out;
        }
        for site in self.text_sites(area) {
            let Some(text) = self.site_text(&site) else {
                continue;
            };
            let chars: Vec<char> = text.chars().collect();
            let macro_chars = macro_flags(&text, chars.len());
            for (start, len) in occurrences(&text, q) {
                out.push(TextHit {
                    site,
                    start,
                    len,
                    in_macro: macro_chars[start],
                    text: text.clone(),
                });
            }
        }
        out
    }

    /// Replaces the character ranges `ranges` of the text at `site` by
    /// `with`. Returns whether the site changed.
    pub fn replace_site_ranges(
        &mut self,
        site: &Site,
        ranges: &[(usize, usize)],
        with: &str,
    ) -> bool {
        if ranges.is_empty() {
            return false;
        }
        let fi = site.floor;
        if fi >= self.floors.len() {
            return false;
        }
        let swap = |s: &mut String| {
            *s = replace_ranges(s, ranges, with);
        };
        match site.field {
            Field::Text => {
                let live = self.macro_texts.texts.iter().position(|t| t.id == site.id);
                if let Some(i) = live {
                    let (runs, rich, facts) = {
                        let t = &self.macro_texts.texts[i];
                        (t.runs.clone(), t.rich, t.facts.clone())
                    };
                    let runs = replace_in_runs(&runs, ranges, with);
                    // The text stays live while it still holds a macro;
                    // otherwise it becomes plain text.
                    if !self.set_live_text(fi, site.id, runs.clone(), rich, facts) {
                        let plain = runs_plain(&runs);
                        self.write_text(fi, site.id, &plain, rich.then_some(runs));
                    }
                    return true;
                }
                let current = self.floors[fi]
                    .cad
                    .iter()
                    .find(|o| o.id == site.id)
                    .and_then(|o| match &o.item {
                        CadItem::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    });
                let Some(current) = current else {
                    return false;
                };
                let attr_runs = self.floors[fi]
                    .cad_attrs(site.id)
                    .map(|a| a.runs)
                    .filter(|r| !r.is_empty());
                let new_text = replace_ranges(&current, ranges, with);
                let new_runs = attr_runs.map(|r| replace_in_runs(&r, ranges, with));
                self.write_text(fi, site.id, &new_text, new_runs);
                // Replacing may have typed a macro: make the text live.
                let runs = self.floors[fi]
                    .cad_attrs(site.id)
                    .map(|a| a.runs)
                    .filter(|r| !r.is_empty());
                let rich = runs.is_some();
                let source = runs.unwrap_or_else(|| vec![RichRun::plain(new_text)]);
                self.set_live_text(fi, site.id, source, rich, Vec::new());
                true
            }
            Field::CalloutLabel => {
                swap(&mut self.floors[fi].annots.callouts[site.index].label);
                self.sync_annotations();
                true
            }
            Field::CalloutBelow => {
                swap(&mut self.floors[fi].annots.callouts[site.index].text_below);
                self.sync_annotations();
                true
            }
            Field::MarkerLabel => {
                swap(&mut self.floors[fi].annots.markers[site.index].label.text);
                self.sync_annotations();
                true
            }
            Field::MarkerBelow => {
                swap(&mut self.floors[fi].annots.markers[site.index].below.text);
                self.sync_annotations();
                true
            }
            Field::NoteLabel => {
                swap(&mut self.floors[fi].annots.notes[site.index].label);
                self.sync_annotations();
                true
            }
            Field::NoteBelow => {
                swap(&mut self.floors[fi].annots.notes[site.index].text_below);
                self.sync_annotations();
                true
            }
            Field::NoteText => {
                swap(&mut self.floors[fi].annots.notes[site.index].text);
                self.sync_annotations();
                true
            }
        }
    }

    fn write_text(&mut self, fi: usize, id: Id, text: &str, runs: Option<Vec<RichRun>>) {
        if let Some(o) = self.floors[fi].cad.iter_mut().find(|o| o.id == id) {
            if let CadItem::Text { text: t, .. } = &mut o.item {
                *t = text.to_string();
            }
        }
        if let Some(runs) = runs {
            self.edit_cad_attrs(fi, id, |a| a.runs = runs);
        }
    }

    /// Replaces one occurrence (Replace). Returns whether it was replaced.
    pub fn replace_hit(&mut self, hit: &TextHit, with: &str) -> bool {
        self.replace_site_ranges(&hit.site, &[(hit.start, hit.len)], with)
    }

    /// Replaces every occurrence of `q.find` in `area` by `q.replace`
    /// (Replace All). Returns `(texts changed, occurrences replaced)`; an
    /// empty search text changes nothing.
    pub fn replace_all_hits(&mut self, q: &TextSearch, area: &Area) -> (usize, usize) {
        if q.find.is_empty() {
            return (0, 0);
        }
        let (mut objects, mut occurrences_replaced) = (0, 0);
        for site in self.text_sites(area) {
            let Some(text) = self.site_text(&site) else {
                continue;
            };
            let found = occurrences(&text, q);
            if found.is_empty() {
                continue;
            }
            if self.replace_site_ranges(&site, &found, &q.replace) {
                objects += 1;
                occurrences_replaced += found.len();
            }
        }
        (objects, occurrences_replaced)
    }

    /// The text objects containing `q.find`, on every floor or on `floor`.
    pub fn find_text(&self, q: &TextSearch, all_floors: bool, floor: usize) -> Vec<TextMatch> {
        let scope = if all_floors {
            Scope::CurrentFile
        } else {
            Scope::CurrentView
        };
        let mut out: Vec<TextMatch> = Vec::new();
        for h in self.find_hits(q, &Area::new(scope, floor, &[])) {
            match out.last_mut() {
                Some(m)
                    if m.floor == h.site.floor
                        && m.id == h.site.id
                        && h.site.field == Field::Text =>
                {
                    m.count += 1
                }
                _ => out.push(TextMatch {
                    floor: h.site.floor,
                    id: h.site.id,
                    text: h.text.clone(),
                    count: 1,
                }),
            }
        }
        out
    }

    /// Replaces `q.find` by `q.replace` in every text in scope. Returns
    /// `(objects changed, occurrences replaced)`.
    pub fn replace_text(
        &mut self,
        q: &TextSearch,
        all_floors: bool,
        floor: usize,
    ) -> (usize, usize) {
        let scope = if all_floors {
            Scope::CurrentFile
        } else {
            Scope::CurrentView
        };
        self.replace_all_hits(q, &Area::new(scope, floor, &[]))
    }

    /// Where a site is, for the Results list: `1st Floor, Text`.
    pub fn site_location(&self, s: &Site) -> String {
        let floor = self
            .floors
            .get(s.floor)
            .map_or("?", |f| f.name.as_str())
            .to_string();
        format!("{floor}, {}", s.field.label())
    }
}

fn macro_flags(text: &str, n: usize) -> Vec<bool> {
    let mut flags = vec![false; n];
    if !text.contains('%') {
        return flags;
    }
    let chars_before = |byte: usize| text[..byte].chars().count();
    for (s, e, _) in tokens(text) {
        for slot in flags.iter_mut().take(chars_before(e)).skip(chars_before(s)) {
            *slot = true;
        }
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::CadItem;
    use crate::geometry::Point;

    fn q(find: &str, replace: &str) -> TextSearch {
        TextSearch {
            find: find.into(),
            replace: replace.into(),
            ..TextSearch::default()
        }
    }

    fn text(p: &mut Project, floor: usize, s: &str) -> Id {
        p.add_cad(
            floor,
            "CAD, Default",
            CadItem::Text {
                pos: Point::ZERO,
                text: s.into(),
                height: 3.0,
                angle: 0.0,
            },
        )
    }

    fn shown(p: &Project, floor: usize, id: Id) -> String {
        match &p.floors[floor]
            .cad
            .iter()
            .find(|o| o.id == id)
            .unwrap()
            .item
        {
            CadItem::Text { text, .. } => text.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn replace_in_honours_case_and_whole_words() {
        let s = "Kitchen kitchenette KITCHEN";
        assert_eq!(
            replace_in(s, &q("kitchen", "Den")),
            ("Den Denette Den".into(), 3)
        );
        let mut cs = q("kitchen", "Den");
        cs.match_case = true;
        assert_eq!(replace_in(s, &cs), ("Kitchen Denette KITCHEN".into(), 1));
        let mut ww = q("kitchen", "Den");
        ww.whole_word = true;
        assert_eq!(replace_in(s, &ww), ("Den kitchenette Den".into(), 2));
        // Non-overlapping, left to right; the replacement is not searched again.
        assert_eq!(replace_in("aaa", &q("aa", "a")), ("aa".into(), 1));
        assert_eq!(replace_in("abc", &q("", "x")), ("abc".into(), 0));
        assert_eq!(count_matches("x x x", &q("x", "")), 3);
        // Multi-byte text keeps its characters.
        assert_eq!(
            replace_in("Küche Küche", &q("küche", "Den")),
            ("Den Den".into(), 2)
        );
    }

    #[test]
    fn percent_signs_and_macro_options() {
        // Expand Percent Signs ignores one or two % in a row.
        let mut e = q("bath", "Suite");
        e.expand_percent = true;
        assert_eq!(count_matches("ba%th and b%%ath and ba%%%th", &e), 2);
        assert_eq!(count_matches("ba%th", &q("bath", "")), 0);
        // The names of macros are searched, not their values.
        let t = "Room %room.name% in the room";
        let mut m = q("room", "X");
        assert_eq!(count_matches(t, &m), 3);
        m.macros = MacroMode::ExcludeMacros;
        assert_eq!(count_matches(t, &m), 2);
        assert_eq!(replace_in(t, &m).0, "X %room.name% in the X");
        m.macros = MacroMode::MacrosOnly;
        assert_eq!(count_matches(t, &m), 1);
        assert_eq!(replace_in(t, &m).0, "Room %X.name% in the room");
        // A percent sign that is not a macro is plain text.
        let mut ex = q("50", "");
        ex.macros = MacroMode::ExcludeMacros;
        assert_eq!(count_matches("50% off and %plan.name%", &ex), 1);
    }

    #[test]
    fn find_and_replace_text_objects_on_one_floor_or_all() {
        let mut p = Project::new("t");
        p.floors.push(crate::model::Floor::new("2nd", 109.0));
        let a = text(&mut p, 0, "Master Bath");
        let _b = text(&mut p, 0, "Garage");
        let c = text(&mut p, 1, "Bath 2");
        let hits = p.find_text(&q("bath", ""), true, 0);
        assert_eq!(hits.iter().map(|m| m.id).collect::<Vec<_>>(), vec![a, c]);
        assert_eq!(p.find_text(&q("bath", ""), false, 0).len(), 1);
        assert_eq!(p.replace_text(&q("Bath", "Suite"), false, 0), (1, 1));
        assert!(
            matches!(&p.floors[0].cad[0].item, CadItem::Text { text, .. } if text == "Master Suite")
        );
        // The other floor is untouched until asked.
        assert!(matches!(&p.floors[1].cad[0].item, CadItem::Text { text, .. } if text == "Bath 2"));
        assert_eq!(p.replace_text(&q("Bath", "Suite"), true, 0), (1, 1));
        assert_eq!(p.replace_text(&q("", "x"), true, 0), (0, 0));
    }

    #[test]
    fn every_scope_finds_its_own_texts() {
        let mut p = Project::new("t");
        p.floors.push(crate::model::Floor::new("2nd", 109.0));
        let a = text(&mut p, 0, "Bath one");
        let b = text(&mut p, 0, "Bath two");
        let c = text(&mut p, 1, "Bath three");
        let s = q("bath", "");
        let n = |scope, sel: &[Id]| p.find_hits(&s, &Area::new(scope, 0, sel)).len();
        assert_eq!(n(Scope::CurrentView, &[]), 2);
        assert_eq!(n(Scope::CurrentFile, &[]), 3);
        assert_eq!(n(Scope::AllOpenFiles, &[]), 3);
        assert_eq!(n(Scope::SelectedObjects, &[b, c]), 2);
        assert_eq!(n(Scope::SelectedObjects, &[]), 0);
        let _ = a;
        // Replace one occurrence, then the rest, in one scope.
        let hits = p.find_hits(&s, &Area::new(Scope::CurrentFile, 0, &[]));
        assert!(p.replace_hit(&hits[1], "Suite"));
        assert_eq!(shown(&p, 0, b), "Suite two");
        assert_eq!(shown(&p, 0, a), "Bath one");
        let (objs, occ) = p.replace_all_hits(
            &q("bath", "Den"),
            &Area::new(Scope::SelectedObjects, 0, &[c]),
        );
        assert_eq!((objs, occ), (1, 1));
        assert_eq!(shown(&p, 1, c), "Den three");
        assert_eq!(shown(&p, 0, a), "Bath one");
    }

    #[test]
    fn rich_text_runs_follow_the_replacement() {
        let mut p = Project::new("t");
        let id = text(&mut p, 0, "Main Bath");
        p.edit_cad_attrs(0, id, |a| {
            a.runs = vec![RichRun::bold("Main "), RichRun::plain("Bath")];
        });
        p.replace_text(&q("Bath", "Suite"), false, 0);
        let runs = p.floors[0].cad_attrs(id).unwrap().runs;
        assert_eq!(runs.len(), 2);
        assert!(runs[0].bold && runs[0].text == "Main ");
        assert_eq!(runs[1].text, "Suite");
        // A match that spans runs collapses them into one with the first style.
        p.replace_text(&q("n Suite", "n Bath"), false, 0);
        let runs = p.floors[0].cad_attrs(id).unwrap().runs;
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "Main Bath");
        assert!(runs[0].bold);
    }

    #[test]
    fn live_texts_are_searched_by_their_macro_names() {
        let mut p = Project::new("Smith");
        let id = text(&mut p, 0, "x");
        assert!(p.set_live_text(
            0,
            id,
            vec![RichRun::plain("Plan %plan.name% for the plan")],
            false,
            Vec::new()
        ));
        assert_eq!(shown(&p, 0, id), "Plan Smith for the plan");
        // The value "Smith" is not found; the macro name is.
        assert_eq!(
            p.find_hits(&q("smith", ""), &Area::new(Scope::CurrentView, 0, &[]))
                .len(),
            0
        );
        let mut m = q("plan", "");
        m.macros = MacroMode::MacrosOnly;
        let hits = p.find_hits(&m, &Area::new(Scope::CurrentView, 0, &[]));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].in_macro);
        // Replacing the macro name keeps the text live and re-evaluates it.
        p.text_macros.add("firm", "DAD");
        let mut r = q("plan.name", "firm");
        r.macros = MacroMode::MacrosOnly;
        assert_eq!(
            p.replace_all_hits(&r, &Area::new(Scope::CurrentView, 0, &[])),
            (1, 1)
        );
        assert_eq!(shown(&p, 0, id), "Plan DAD for the plan");
        assert!(p.live_text(id).is_some());
        // Replacing plain text edits the source.
        p.replace_all_hits(
            &q("for the plan", "for %plan.name%"),
            &Area::new(Scope::CurrentView, 0, &[]),
        );
        assert_eq!(shown(&p, 0, id), "Plan DAD for Smith");
    }

    #[test]
    fn annotation_records_are_searched_not_their_generated_text() {
        use crate::callout::{Callout, Note};
        let mut p = Project::new("t");
        let mut c = Callout::default();
        c.label = "A1".into();
        c.text_below = "SEE BATH".into();
        p.add_callout(0, c);
        let mut n = Note::default();
        n.text = "Verify bath size".into();
        n.label = "N1".into();
        p.add_note(0, n);
        p.sync_annotations();
        let hits = p.find_hits(&q("bath", ""), &Area::new(Scope::CurrentView, 0, &[]));
        // Once in the callout, once in the note's schedule text: never twice
        // through the generated CAD text.
        assert_eq!(hits.len(), 2, "{hits:?}");
        let (objs, occ) =
            p.replace_all_hits(&q("bath", "Suite"), &Area::new(Scope::CurrentView, 0, &[]));
        assert_eq!((objs, occ), (2, 2));
        assert_eq!(p.floors[0].annots.callouts[0].text_below, "SEE Suite");
        assert_eq!(p.floors[0].annots.notes[0].text, "Verify Suite size");
        let again = p.find_hits(&q("bath", ""), &Area::new(Scope::CurrentView, 0, &[]));
        assert!(again.is_empty());
        assert_eq!(
            p.site_location(&hits[0].site),
            "1st Floor, Callout text below line"
        );
    }
}
