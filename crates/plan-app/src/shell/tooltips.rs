//! One-line tooltip descriptions for toolbar buttons, taken from the command
//! tables of the user manual (`docs/manual`): any table whose first column is
//! Button, Variant, Command or Tool and that has a "What it does" style column.
//! The tables are read once, at first use.

use std::collections::HashMap;
use std::sync::OnceLock;

const CHAPTERS: [&str; 12] = [
    include_str!("../../../../docs/manual/02-walls.md"),
    include_str!("../../../../docs/manual/03-doors-windows.md"),
    include_str!("../../../../docs/manual/04-rooms-floors.md"),
    include_str!("../../../../docs/manual/05-dimensions-text-cad.md"),
    include_str!("../../../../docs/manual/06-cabinets-library.md"),
    include_str!("../../../../docs/manual/07-stairs.md"),
    include_str!("../../../../docs/manual/08-roofs.md"),
    include_str!("../../../../docs/manual/09-electrical-terrain.md"),
    include_str!("../../../../docs/manual/10-3d-views-rendering.md"),
    include_str!("../../../../docs/manual/11-layout-schedules-print.md"),
    include_str!("../../../../docs/manual/16-foundation-slabs.md"),
    include_str!("../../../../docs/manual/17-exterior-details.md"),
];

/// Longest description shown, in characters.
const MAX_LEN: usize = 110;

/// Column headings (lower case) that hold a description.
const DESCRIPTION_HEADINGS: [&str; 6] = [
    "what it does",
    "does",
    "today",
    "what a click does",
    "how it works",
    "what it makes",
];

/// The one-line description of the command called `name`, if the manual has one.
pub fn describe(name: &str) -> Option<&'static str> {
    table().get(&normalize(name)).map(String::as_str)
}

fn table() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for chapter in CHAPTERS {
            read_tables(chapter, &mut map);
        }
        map
    })
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_rule(line: &str) -> bool {
    let l = line.trim();
    l.starts_with('|') && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn read_tables(text: &str, map: &mut HashMap<String, String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i + 1 < lines.len() {
        if !(lines[i].starts_with('|') && is_rule(lines[i + 1])) {
            i += 1;
            continue;
        }
        let header: Vec<String> = cells(lines[i]).iter().map(|h| h.to_lowercase()).collect();
        let col = header
            .iter()
            .position(|h| DESCRIPTION_HEADINGS.contains(&h.as_str()));
        let named = matches!(
            header[0].as_str(),
            "button" | "variant" | "command" | "tool"
        );
        i += 2;
        while i < lines.len() && lines[i].starts_with('|') {
            if let (true, Some(col)) = (named, col) {
                let row = cells(lines[i]);
                if let Some(desc) = row.get(col).and_then(|d| first_sentence(d)) {
                    for name in row[0].split(", ") {
                        map.entry(normalize(name)).or_insert_with(|| desc.clone());
                    }
                }
            }
            i += 1;
        }
    }
}

/// Lower-cases `name` and drops markdown, a trailing "(`hotkey`)" and a
/// leading "**Group** flyout:".
fn normalize(name: &str) -> String {
    let mut s = name.replace("**", "").replace('`', "");
    if let Some(rest) = s.split_once(" flyout: ").map(|(_, r)| r.to_string()) {
        s = rest;
    }
    if let Some(p) = s.rfind(" (") {
        if s.ends_with(')') {
            s.truncate(p);
        }
    }
    s.trim().trim_end_matches("...").to_lowercase()
}

/// The first real sentence of a table cell: the leading "Works." status word
/// is skipped and markdown removed. `None` when nothing informative is left.
fn first_sentence(cell: &str) -> Option<String> {
    let clean = cell.replace("**", "").replace('`', "");
    let mut rest = clean.trim();
    for status in ["Works.", "Work.", "Works,", "Works "] {
        if let Some(r) = rest.strip_prefix(status) {
            // "Works from the toolbar and menu." says nothing about the command.
            if status == "Works " {
                return None;
            }
            rest = r.trim();
            break;
        }
    }
    let end = rest
        .match_indices(". ")
        .map(|(i, _)| i + 1)
        .next()
        .unwrap_or(rest.len());
    let mut s = rest[..end].trim().to_string();
    if s.len() < 8 || s.starts_with("Not yet") || s.starts_with("Same") {
        return None;
    }
    if s.chars().count() > MAX_LEN {
        s = s.chars().take(MAX_LEN - 1).collect::<String>() + "\u{2026}";
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_tables_give_one_line_descriptions() {
        assert_eq!(
            describe("Straight Exterior Wall"),
            Some("Uses the Default Settings exterior wall type and height.")
        );
        assert_eq!(
            describe("Down One Floor"),
            Some("Moves the view to the floor below.")
        );
        // The "(`Cmd+Q`)" hotkey in the manual's name cell is not part of the name.
        assert!(describe("Straight Railing").is_none());
        // "Works." alone is not a description.
        assert!(describe("Curved Exterior Wall").is_none());
        // A comma-joined name cell describes both.
        assert!(describe("nothing like this").is_none());
        assert!(describe("Perspective Full Overview").is_some());
    }

    #[test]
    fn descriptions_are_single_short_lines() {
        let t = table();
        assert!(t.len() > 40, "only {} descriptions", t.len());
        for (name, d) in t {
            assert!(
                !d.contains('\n') && !d.contains('*') && !d.contains('`'),
                "{name}"
            );
            assert!(d.chars().count() <= MAX_LEN, "{name}: {d}");
        }
    }

    #[test]
    fn sentences_skip_the_status_word() {
        assert_eq!(first_sentence("Works."), None);
        assert_eq!(
            first_sentence("Works. Uses the interior wall defaults."),
            Some("Uses the interior wall defaults.".into())
        );
        assert_eq!(
            first_sentence("Works from the toolbar and menu. The key is shown."),
            None
        );
        assert_eq!(
            first_sentence("Opens the Build New Floor dialog (4.5)."),
            Some("Opens the Build New Floor dialog (4.5).".into())
        );
    }
}
