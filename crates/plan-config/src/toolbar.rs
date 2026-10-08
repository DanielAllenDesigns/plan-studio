//! Chief `.toolbar` files ("Chief Toolbar File: 9.0").
//!
//! The format, reverse-engineered from the four files in
//! `docs/chief-config-raw/` (see the crate README for the full write-up):
//!
//! ```text
//! Chief Toolbar File: 9.0                  line 0: magic + format version
//! Chief Architect Premier<TAB>  30         line 1: product, file revision
//! 000000ff00000000fd...                    line 2: hex dump of a Qt QMainWindow::saveState() blob
//! <name><TAB>                              then, per toolbar, three lines:
//!    0    1    3   14                       - view types the toolbar shows in (may be empty)
//!  359 20221 20218 ...                       - command ids, left to right (may be empty)
//! ...
//! <blank>
//! Buttons                                  marker
//!  101 &New Plan 102 &Open Plan... ...     one line: the id -> label table
//! ```
//!
//! The label table is a concatenation of `%4d <label>` records with no other
//! separator, so a label runs until the next numeric id. Ids are stored in
//! ascending order, which is what lets the reader find the boundary even when
//! a label itself contains a digit (`3D Solid Tools`).
//!
//! The Qt blob carries each toolbar's dock area, row and position. Separators
//! between buttons are not stored in the file at all, so
//! [`ToolbarItemDef::separator_before`] is always `false` when parsed.

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where a toolbar docks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Placement {
    Top,
    Right,
    Bottom,
    Left,
    /// Not present in the saved window layout.
    Floating,
}

/// One button (or flyout group) on a toolbar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolbarItemDef {
    /// Chief command id as text. Empty for flyout members whose id is unknown.
    pub command_id: String,
    /// Label from the file's name table, with `&` mnemonics and `...` removed.
    pub command_name: Option<String>,
    /// Flyout variants. The `.toolbar` file does not list them; they are added
    /// from the captured sub-tool inventory by [`crate::load_daniel_config`].
    pub flyout: Vec<ToolbarItemDef>,
    pub separator_before: bool,
}

/// One toolbar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolbarDef {
    pub name: String,
    pub placement: Placement,
    /// Row within the dock area (0 = outermost line), from the Qt layout.
    pub row: usize,
    /// Offset along the row in pixels, from the Qt layout.
    pub pos: i32,
    /// Shown in Chief's default window state (Qt's "shown" bit).
    pub visible: bool,
    /// Raw Qt per-toolbar flag byte, kept for reference.
    pub state_flags: u8,
    /// Chief view-type codes the toolbar appears in (empty = contextual).
    pub view_types: Vec<u32>,
    pub items: Vec<ToolbarItemDef>,
}

/// One `.toolbar` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolbarSet {
    pub name: String,
    pub product: String,
    /// Header line 1 number (30 in the X18 files).
    pub file_revision: u32,
    /// Toolbars in file order (alphabetical, as Chief writes them).
    pub toolbars: Vec<ToolbarDef>,
    /// Command id -> cleaned label, from the `Buttons` table.
    pub button_names: BTreeMap<String, String>,
}

impl ToolbarSet {
    pub fn find(&self, name: &str) -> Option<&ToolbarDef> {
        self.toolbars.iter().find(|t| t.name == name)
    }

    /// Toolbars sorted the way they appear on screen: by dock area, then row,
    /// then position.
    pub fn layout_order(&self) -> Vec<&ToolbarDef> {
        let mut v: Vec<&ToolbarDef> = self.toolbars.iter().collect();
        v.sort_by_key(|t| (placement_rank(t.placement), t.row, t.pos));
        v
    }

    /// The Build tools toolbar (`Architectural Features` in the Default set,
    /// `Architectural Tools`, `Space Planning Features` or `Terrain Features`
    /// in the others).
    pub fn build_toolbar(&self) -> Option<&ToolbarDef> {
        [
            "Architectural Features",
            "Architectural Tools",
            "Space Planning Features",
            "Terrain Features",
        ]
        .iter()
        .find_map(|n| self.find(n))
    }
}

fn placement_rank(p: Placement) -> u8 {
    match p {
        Placement::Top => 0,
        Placement::Left => 1,
        Placement::Right => 2,
        Placement::Bottom => 3,
        Placement::Floating => 4,
    }
}

/// Strips mnemonic ampersands, trailing ellipses and the `[hint]` suffix Chief
/// appends to some labels.
pub fn clean_label(raw: &str) -> String {
    let mut s = raw.replace('&', "");
    if let Some(i) = s.find(" [") {
        if s.ends_with(']') {
            s.truncate(i);
        }
    }
    let s = s.trim();
    let s = s
        .strip_suffix("...")
        .or_else(|| s.strip_suffix('\u{2026}'))
        .unwrap_or(s);
    s.trim().to_string()
}

/// Parses a `.toolbar` file. The set name is inferred from the toolbars it
/// contains; use [`parse_toolbar_named`] when the file name is known.
pub fn parse_toolbar(text: &str) -> Result<ToolbarSet, ConfigError> {
    let mut set = parse_toolbar_named("", text)?;
    set.name = infer_set_name(&set).to_string();
    Ok(set)
}

fn infer_set_name(set: &ToolbarSet) -> &'static str {
    if set.find("Space Planning Features").is_some() {
        "Space Planning Configuration"
    } else if set.find("Terrain Features").is_some() {
        "Terrain Configuration"
    } else if set.find("Architectural Tools").is_some() {
        "Extended Tool Configuration"
    } else if set.find("Architectural Features").is_some() {
        "Default Configuration"
    } else {
        "Unnamed Configuration"
    }
}

/// Parses a `.toolbar` file and gives the set an explicit name.
pub fn parse_toolbar_named(name: &str, text: &str) -> Result<ToolbarSet, ConfigError> {
    let lines: Vec<&str> = text.lines().collect();
    let magic = lines.first().copied().unwrap_or("");
    if !magic.starts_with("Chief Toolbar File:") {
        return Err(ConfigError::Format(
            "not a Chief toolbar file (missing 'Chief Toolbar File:' header)".into(),
        ));
    }
    let buttons_at = lines
        .iter()
        .position(|l| l.trim_end() == "Buttons")
        .ok_or_else(|| ConfigError::Format("toolbar file has no 'Buttons' table".into()))?;
    if buttons_at < 3 {
        return Err(ConfigError::Format(
            "toolbar file header is truncated".into(),
        ));
    }

    let (product, rev) = {
        let mut it = lines[1].splitn(2, '\t');
        let p = it.next().unwrap_or("").trim().to_string();
        let r = it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        (p, r)
    };
    let layout = decode_layout(lines[2]);
    let button_names = parse_button_table(lines.get(buttons_at + 1).copied().unwrap_or(""));

    let mut toolbars = Vec::new();
    let mut i = 3;
    while i < buttons_at {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let Some(tb_name) = line.strip_suffix('\t') else {
            return Err(ConfigError::Format(format!(
                "expected a toolbar name ending in a tab at line {}",
                i + 1
            )));
        };
        let views = lines.get(i + 1).copied().unwrap_or("");
        let items = lines.get(i + 2).copied().unwrap_or("");
        i += 3;

        let view_types = views
            .split_whitespace()
            .filter_map(|t| t.parse::<u32>().ok())
            .collect();
        let items = items
            .split_whitespace()
            .map(|id| ToolbarItemDef {
                command_id: id.to_string(),
                command_name: button_names.get(id).cloned(),
                flyout: Vec::new(),
                separator_before: false,
            })
            .collect();
        let placed = layout.iter().find(|e| e.name == tb_name);
        toolbars.push(ToolbarDef {
            name: tb_name.to_string(),
            placement: placed.map_or(Placement::Floating, |e| e.placement),
            row: placed.map_or(0, |e| e.row),
            pos: placed.map_or(0, |e| e.pos),
            visible: placed.is_some_and(|e| e.flags & 1 != 0),
            state_flags: placed.map_or(0, |e| e.flags),
            view_types,
            items,
        });
    }

    Ok(ToolbarSet {
        name: name.to_string(),
        product,
        file_revision: rev,
        toolbars,
        button_names,
    })
}

/// Parses the concatenated `%4d label` records.
fn parse_button_table(line: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let b = line.as_bytes();
    let mut pos = 0;
    while pos < b.len() {
        // Skip padding, read the id.
        while pos < b.len() && b[pos] == b' ' {
            pos += 1;
        }
        let id_start = pos;
        while pos < b.len() && b[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == id_start {
            break;
        }
        let id: u64 = line[id_start..pos].parse().unwrap_or(0);
        if pos < b.len() && b[pos] == b' ' {
            pos += 1;
        }
        // The label ends where the next record starts: the earliest position
        // that begins a record whose id is larger than this one.
        let mut end = b.len();
        let mut j = pos;
        while j < b.len() {
            if let Some((next_id, _)) = record_start(line, j) {
                if next_id > id {
                    end = j;
                    break;
                }
            }
            j += 1;
        }
        let label = line[pos..end].to_string();
        out.insert(id.to_string(), clean_label(&label));
        pos = end;
    }
    out
}

/// Does a record start at byte `j`? Returns the id and the byte after the id.
/// A record is either 1-3 digits padded by spaces to width four, or four/five
/// digits with no padding, followed by a space.
fn record_start(line: &str, j: usize) -> Option<(u64, usize)> {
    let b = line.as_bytes();
    let mut k = j;
    let mut pad = 0;
    while k < b.len() && b[k] == b' ' && pad < 3 {
        k += 1;
        pad += 1;
    }
    let ds = k;
    while k < b.len() && b[k].is_ascii_digit() {
        k += 1;
    }
    let digits = k - ds;
    if digits == 0 || k >= b.len() || b[k] != b' ' {
        return None;
    }
    // Width-four field: padding + digits must be exactly four (or the id is
    // wider than the field, in which case there is no padding).
    let ok = if pad == 0 {
        digits >= 4
    } else {
        pad + digits == 4
    };
    if !ok {
        return None;
    }
    line[ds..k].parse().ok().map(|id| (id, k))
}

struct LayoutEntry {
    name: String,
    placement: Placement,
    row: usize,
    pos: i32,
    flags: u8,
}

/// Decodes the Qt `QMainWindow::saveState()` blob far enough to recover each
/// toolbar's dock, row, offset and shown flag. Anything unexpected yields an
/// empty layout (every toolbar then reads as floating).
fn decode_layout(hex: &str) -> Vec<LayoutEntry> {
    let digits: Vec<u8> = hex
        .trim()
        .bytes()
        .filter(u8::is_ascii_hexdigit)
        .map(|c| match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => c - b'A' + 10,
        })
        .collect();
    if !digits.len().is_multiple_of(2) || digits.is_empty() {
        return Vec::new();
    }
    let data: Vec<u8> = digits.chunks(2).map(|p| p[0] << 4 | p[1]).collect();
    decode_layout_bytes(&data).unwrap_or_default()
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.d.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn i32(&mut self) -> Option<i32> {
        let s = self.d.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn qstring(&mut self) -> Option<String> {
        let n = self.i32()?;
        if n < 0 {
            return Some(String::new());
        }
        let n = n as usize;
        let s = self.d.get(self.p..self.p + n)?;
        self.p += n;
        let units: Vec<u16> = (0..s.len() / 2)
            .map(|i| u16::from_be_bytes([s[2 * i], s[2 * i + 1]]))
            .collect();
        Some(String::from_utf16_lossy(&units))
    }
}

fn decode_layout_bytes(d: &[u8]) -> Option<Vec<LayoutEntry>> {
    // The toolbar section starts at the 0xFC ("ToolBarStateMarkerEx") byte that
    // follows the dock-widget section; its first field is a line count.
    let start = d.iter().position(|&b| b == 0xFC)?;
    let mut r = Reader { d, p: start + 1 };
    let lines = r.i32()?;
    let mut out = Vec::new();
    let mut rows_per_dock = [0usize; 4];
    for _ in 0..lines {
        let dock = r.i32()?;
        let count = r.i32()?;
        let placement = match dock {
            0 => Placement::Left,
            1 => Placement::Right,
            2 => Placement::Top,
            3 => Placement::Bottom,
            _ => Placement::Floating,
        };
        let row = rows_per_dock.get(dock as usize).copied().unwrap_or(0);
        if let Some(slot) = rows_per_dock.get_mut(dock as usize) {
            *slot += 1;
        }
        for _ in 0..count {
            let name = r.qstring()?;
            let flags = r.u8()?;
            let pos = r.i32()?;
            let _size = r.i32()?;
            let _reserved_a = r.i32()?;
            let _reserved_b = r.i32()?;
            out.push(LayoutEntry {
                name,
                placement,
                row,
                pos,
                flags,
            });
        }
    }
    Some(out)
}
