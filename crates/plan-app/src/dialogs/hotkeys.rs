//! Customize Hotkeys, modeled on Chief's dialog: a "Show Commands/Hotkeys
//! Containing" filter, a Command Name | Hotkey table, and an "Assign a
//! sequence of up to 4 hotkeys" field with Assign and Remove, then Reset
//! Hotkeys and Help / Cancel / OK.
//!
//! The dialog edits a copy of the [`HotkeyMap`]; OK writes it back and saves
//! `~/.plan-studio/hotkeys.json`, Cancel drops it.
//!
//! On top of Chief's layout it has: a search that matches command, group and
//! key; a filter (all, assigned, unassigned, in conflict); a live conflict
//! warning while a sequence is being recorded and a list of every clash in the
//! map; Import of a Chief `UserHotkeys.xml` (again, over the current keys);
//! Export of the keys as JSON or CSV; and Print List, which writes the
//! assigned keys as a two-column PDF and opens it in the system viewer.
//!
//! Round 14 adds: the list grouped by Chief's menus (File, Edit, Build ...)
//! with the search opening the groups it hits; a "Collide on Windows and
//! Linux" list, where Control and Command are one key and two sequences that
//! differ on the Mac land on the same key; one-click conflict resolution
//! (keep the key for one command, take it from the others); Export of Chief's
//! own `UserHotkeys.xml`; and two resets, Chief's defaults and Daniel's file.

use super::Outcome;
use crate::shell::hotkeys::{
    sequence_label, Chord, Command, ExportStats, FoldedCollision, HotkeyMap, MAX_SEQUENCE,
};
use eframe::egui::{self, Align, Align2, Color32, Event, Key, Layout, RichText, Vec2};
use std::collections::{HashMap, HashSet};

const WARN: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);

/// What pressing Assign did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignResult {
    Assigned,
    /// The sequence clashes with these commands; nothing changed.
    Conflict(Vec<String>),
    /// No command selected or no keys recorded.
    Nothing,
}

/// Which commands the table lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Assigned,
    Unassigned,
    Conflicts,
}

impl FilterMode {
    pub const ALL: [FilterMode; 4] = [
        FilterMode::All,
        FilterMode::Assigned,
        FilterMode::Unassigned,
        FilterMode::Conflicts,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FilterMode::All => "All",
            FilterMode::Assigned => "Assigned",
            FilterMode::Unassigned => "Unassigned",
            FilterMode::Conflicts => "In conflict",
        }
    }
}

/// Commands with the key sequence of each that is in play.
pub type Owners = Vec<(String, Vec<Chord>)>;

/// Commands whose keys clash: the same sequence, or one that is the start of
/// another (`D` and `D, H`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The shorter (or shared) sequence, as shown to the user.
    pub sequence: String,
    /// The commands involved, by name.
    pub commands: Vec<String>,
    /// Each command with the sequence of its that clashes.
    pub owners: Owners,
}

/// Every clash in `map`.
pub fn find_conflicts(map: &HotkeyMap) -> Vec<Conflict> {
    let mut owners: HashMap<Vec<Chord>, Vec<&str>> = HashMap::new();
    for c in map.commands() {
        for seq in map.sequences(&c.name) {
            owners.entry(seq.clone()).or_default().push(&c.name);
        }
    }
    let mut out: Vec<Conflict> = Vec::new();
    // `parts` are (command, its sequence) pairs of the clash.
    let mut add = |sequence: String, parts: Vec<(String, Vec<Chord>)>| {
        let mut names: Vec<String> = parts.iter().map(|(n, _)| n.clone()).collect();
        names.sort();
        names.dedup();
        if names.len() > 1
            && !out
                .iter()
                .any(|c| c.sequence == sequence && c.commands == names)
        {
            let mut parts = parts;
            parts.sort_by(|a, b| a.0.cmp(&b.0));
            parts.dedup();
            out.push(Conflict {
                sequence,
                commands: names,
                owners: parts,
            });
        }
    };
    for (seq, names) in &owners {
        // The same sequence on two commands.
        add(
            sequence_label(seq),
            names.iter().map(|n| (n.to_string(), seq.clone())).collect(),
        );
        // This sequence starts with another command's sequence.
        for n in 1..seq.len() {
            if let Some(short) = owners.get(&seq[..n]) {
                let mut all: Vec<(String, Vec<Chord>)> = short
                    .iter()
                    .map(|c| (c.to_string(), seq[..n].to_vec()))
                    .collect();
                all.extend(names.iter().map(|c| (c.to_string(), seq.clone())));
                add(sequence_label(&seq[..n]), all);
            }
        }
    }
    out.sort_by(|a, b| a.sequence.cmp(&b.sequence));
    out
}

/// The top-level menus of Chief X18 that hold commands, in menu-bar order.
/// Commands Plan Studio has that sit in no menu of Chief's (toolbar-only
/// buttons) are listed under the last entry.
pub const MENU_ORDER: [&str; 12] = [
    "File", "Edit", "Build", "Terrain", "Library", "3D", "CAD", "Layout", "View", "Window",
    "Tools", "Help",
];

/// The Chief menu a command belongs to. Plan Studio's menus are code, not
/// data, so this reads the command's flyout group and name against the
/// menu contents recorded in `docs/chief-x18-menus.md`; it is a close guess,
/// not a mirror (verify in Chief).
pub fn menu_of(c: &Command) -> &'static str {
    let name = c.name.to_lowercase();
    let group = c.group.to_lowercase();
    let has = |hay: &str, words: &[&str]| words.iter().any(|w| hay.contains(w));
    let both = format!("{group} {name}");
    if has(&name, &["help", "about", "release notes"]) {
        return "Help";
    }
    if has(
        &name,
        &[
            "quit",
            "new plan",
            "open plan",
            "save",
            "print",
            "export",
            "import",
            "recent",
            "close view",
            "send to layout",
            "template",
            "archive",
        ],
    ) && !has(&name, &["view", "saved view"])
        || group == "file"
    {
        return "File";
    }
    if has(&both, &["layout", "page ", "sheet", "plot"]) && !has(&name, &["send to"]) {
        return "Layout";
    }
    if has(
        &group,
        &[
            "terrain",
            "road",
            "driveway",
            "sidewalk",
            "plant",
            "sprinkler",
            "garden",
            "grass",
            "water feature",
            "stepping",
        ],
    ) || has(&name, &["terrain", "elevation data"])
    {
        return "Terrain";
    }
    if has(&both, &["library", "catalog"]) {
        return "Library";
    }
    if has(
        &both,
        &[
            "camera",
            "3d view",
            "ray trace",
            "render",
            "perspective",
            "overview",
            "walkthrough",
            "orbit",
            "glass house",
            "adjust lights",
            "full camera",
            "sun angle",
        ],
    ) {
        return "3D";
    }
    if has(
        &group,
        &[
            "point",
            "line",
            "arc",
            "circle",
            "box",
            "dimension",
            "text",
            "cad",
            "spline",
            "cloud",
            "block",
            "detail",
            "polyline",
            "symbol",
        ],
    ) || has(
        &name,
        &[
            "cad ",
            "dimension",
            "revision cloud",
            "spline",
            "auto detail",
        ],
    ) {
        return "CAD";
    }
    if group == "edit"
        || has(
            &name,
            &[
                "undo",
                "redo",
                "cut",
                "copy",
                "paste",
                "delete",
                "select",
                "duplicate",
                "group",
                "snap",
                "align",
                "mirror",
                "move",
                "rotate",
                "resize",
                "replicate",
                "transform",
                "find",
                "stretch",
                "default settings",
                "reset to defaults",
            ],
        )
    {
        return "Edit";
    }
    if has(
        &name,
        &[
            "zoom",
            "pan",
            "fit",
            "grid",
            "toolbar",
            "status bar",
            "full screen",
            "view",
            "layer",
            "display",
            "reference",
            "label",
        ],
    ) || group == "view"
    {
        return "View";
    }
    if has(&name, &["tile windows", "cascade", "browser", "dock"]) {
        return "Window";
    }
    if has(
        &group,
        &[
            "wall",
            "door",
            "window",
            "floor",
            "roof",
            "slab",
            "framing",
            "trim",
            "stair",
            "cabinet",
            "electrical",
            "railing",
            "fenc",
            "deck",
            "3d solid",
            "image",
            "distributed",
            "room",
            "ceiling",
            "foundation",
            "build",
            "dormer",
            "column",
            "beam",
            "post",
            "appliance",
            "fixture",
            "furniture",
        ],
    ) || has(
        &name,
        &[
            "wall",
            "door",
            "window",
            "floor",
            "roof",
            "stair",
            "cabinet",
            "room",
            "ceiling",
            "foundation",
            "railing",
            "deck",
            "framing",
        ],
    ) {
        return "Build";
    }
    "Tools"
}

/// What an import of a Chief `UserHotkeys.xml` did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotkeyImport {
    /// Bound commands in the file that carry a name.
    pub bindings: usize,
    /// Plan Studio commands whose keys were set from them.
    pub commands: usize,
    /// Named bindings with no Plan Studio command (or no usable keys).
    pub unmapped: usize,
    /// Bound commands whose name could not be recovered from the file.
    pub unnamed: usize,
}

impl HotkeyImport {
    pub fn summary(&self) -> String {
        format!(
            "Imported {} bindings onto {} commands ({} have no Plan Studio command yet, {} unnamed)",
            self.bindings, self.commands, self.unmapped, self.unnamed
        )
    }
}

pub struct HotkeyDialog {
    draft: HotkeyMap,
    filter_mode: FilterMode,
    filter: String,
    selected: Option<String>,
    /// The sequence being recorded (up to [`MAX_SEQUENCE`] chords).
    seq: Vec<Chord>,
    recording: bool,
    /// Commands the recorded sequence clashes with, once Assign was pressed.
    conflict: Option<Vec<String>>,
    /// The assigned sequence picked for Remove.
    chosen: Option<usize>,
    message: String,
    /// List the commands under Chief's menus instead of one flat table.
    grouped: bool,
}

impl HotkeyDialog {
    pub fn new(map: &HotkeyMap) -> Self {
        HotkeyDialog {
            draft: map.clone(),
            filter_mode: FilterMode::All,
            filter: String::new(),
            selected: None,
            seq: Vec::new(),
            recording: false,
            conflict: None,
            chosen: None,
            message: String::new(),
            grouped: true,
        }
    }

    /// The visible commands under Chief's menus, in menu-bar order; a menu
    /// with none left by the search is left out.
    pub fn grouped_commands(&self) -> Vec<(&'static str, Vec<&Command>)> {
        let visible = self.visible_commands();
        MENU_ORDER
            .iter()
            .filter_map(|menu| {
                let cmds: Vec<&Command> = visible
                    .iter()
                    .copied()
                    .filter(|c| menu_of(c) == *menu)
                    .collect();
                (!cmds.is_empty()).then_some((*menu, cmds))
            })
            .collect()
    }

    /// Sequences that are two keys here and one key without a Command key.
    pub fn folded_collisions(&self) -> Vec<FoldedCollision> {
        self.draft.folded_collisions()
    }

    /// Resolves a clash by keeping the key for `keep`: the clashing
    /// sequences of the other commands are removed. Returns how many.
    pub fn resolve(&mut self, owners: &[(String, Vec<Chord>)], keep: &str) -> usize {
        let mut removed = 0;
        for (name, seq) in owners {
            if name != keep && self.draft.sequences(name).contains(seq) {
                self.draft.remove(name, seq);
                removed += 1;
            }
        }
        self.chosen = None;
        self.conflict = None;
        self.message = format!("Kept the key for {keep}; took it from {removed} other command(s)");
        removed
    }

    /// Resolves every clash in the map in favor of the command that comes
    /// first in the clash (by name). Returns how many sequences were removed.
    pub fn resolve_all(&mut self) -> usize {
        let mut removed = 0;
        let found = find_conflicts(&self.draft);
        for c in &found {
            if let Some(keep) = c.commands.first().cloned() {
                removed += self.resolve(&c.owners, &keep);
            }
        }
        self.message = format!(
            "Settled {} clash(es) by taking {removed} key(s) from the later commands",
            found.len()
        );
        removed
    }

    /// Reset to Chief's own defaults, without Daniel's `UserHotkeys.xml`.
    pub fn reset_to_chief(&mut self) {
        self.draft.reset_to_chief_defaults();
        self.chosen = None;
        self.conflict = None;
        self.message = "Hotkeys reset to Chief's defaults".into();
    }

    /// The keys as a Chief `UserHotkeys.xml`, and what it could not carry.
    pub fn export_chief_xml(&self) -> (String, ExportStats) {
        self.draft.to_chief_xml()
    }

    /// The hotkeys of a command in the edited map, as the table shows them.
    #[cfg(test)]
    pub fn draft_hotkeys(&self, name: &str) -> String {
        self.draft.hotkey_text(name)
    }

    /// Is the edited map what Reset gives (Chief plus Daniel's file)?
    pub fn is_default(&self) -> bool {
        self.draft.is_default()
    }

    /// The commands whose name, group or hotkey contains the filter and that
    /// pass the filter mode, by name.
    pub fn visible_commands(&self) -> Vec<&Command> {
        let needle = self.filter.trim().to_lowercase();
        let in_conflict: HashSet<String> = if self.filter_mode == FilterMode::Conflicts {
            find_conflicts(&self.draft)
                .into_iter()
                .flat_map(|c| c.commands)
                .collect()
        } else {
            HashSet::new()
        };
        let mut v: Vec<&Command> = self
            .draft
            .commands()
            .iter()
            .filter(|c| {
                needle.is_empty()
                    || c.name.to_lowercase().contains(&needle)
                    || c.group.to_lowercase().contains(&needle)
                    || self
                        .draft
                        .hotkey_text(&c.name)
                        .to_lowercase()
                        .contains(&needle)
            })
            .filter(|c| match self.filter_mode {
                FilterMode::All => true,
                FilterMode::Assigned => !self.draft.sequences(&c.name).is_empty(),
                FilterMode::Unassigned => self.draft.sequences(&c.name).is_empty(),
                FilterMode::Conflicts => in_conflict.contains(&c.name),
            })
            .collect();
        v.sort_by_key(|c| c.name.to_lowercase());
        v
    }

    #[cfg(test)]
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_string();
    }

    #[cfg(test)]
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_mode = mode;
    }

    /// Every clash in the edited map.
    #[cfg(test)]
    pub fn conflicts(&self) -> Vec<Conflict> {
        find_conflicts(&self.draft)
    }

    /// Commands (other than the selected one) that the sequence being
    /// recorded would clash with, before Assign is pressed.
    pub fn pending_conflicts(&self) -> Vec<String> {
        match (&self.selected, self.seq.is_empty()) {
            (Some(cmd), false) => self.draft.conflicts(cmd, &self.seq),
            _ => Vec::new(),
        }
    }

    // ----- import, export, print -----

    /// Reads a Chief `UserHotkeys.xml` and sets the keys of every command it
    /// binds that Plan Studio has: the command's current keys are replaced by
    /// the file's, taking a key from any other command that holds it. Commands
    /// the file does not bind keep what they have. (Plan Studio's own extra
    /// keys on an imported command, such as the number keys, are replaced
    /// too; Reset Hotkeys brings them back.)
    pub fn import_chief_xml(&mut self, text: &str) -> Result<HotkeyImport, String> {
        let (file, _) = plan_config::import_hotkeys_xml(text).map_err(|e| e.to_string())?;
        let mut out = HotkeyImport {
            unnamed: file.unnamed_count(),
            ..HotkeyImport::default()
        };
        let mut by_command: Vec<(String, Vec<Vec<Chord>>)> = Vec::new();
        for pb in plan_config::to_plan_studio_bindings(&file) {
            out.bindings += 1;
            let seq: Option<Vec<Chord>> = pb.keys.iter().map(Chord::from_config).collect();
            let name = self.draft.command(&pb.command_name).map(|c| c.name.clone());
            match (name, seq) {
                (Some(name), Some(seq)) if !seq.is_empty() && seq.len() <= MAX_SEQUENCE => {
                    match by_command.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, list)) => list.push(seq),
                        None => by_command.push((name, vec![seq])),
                    }
                }
                _ => out.unmapped += 1,
            }
        }
        for (name, _) in &by_command {
            for seq in self.draft.sequences(name).to_vec() {
                self.draft.remove(name, &seq);
            }
        }
        for (name, seqs) in &by_command {
            for seq in seqs {
                let _ = self.draft.assign(name, seq.clone(), true);
            }
        }
        out.commands = by_command.len();
        self.chosen = None;
        self.conflict = None;
        self.message = out.summary();
        Ok(out)
    }

    /// The edited keys as `hotkeys.json` stores them (only the differences
    /// from the defaults).
    pub fn export_json(&self) -> String {
        self.draft.overrides_json()
    }

    /// `(group, command, hotkeys)` of every command with a key, by group then
    /// name.
    pub fn list_rows(&self) -> Vec<(String, String, String)> {
        let mut rows: Vec<(String, String, String)> = self
            .draft
            .commands()
            .iter()
            .filter(|c| !self.draft.sequences(&c.name).is_empty())
            .map(|c| {
                (
                    c.group.clone(),
                    c.name.clone(),
                    self.draft.hotkey_text(&c.name),
                )
            })
            .collect();
        rows.sort_by(|a, b| {
            (a.0.to_lowercase(), a.1.to_lowercase()).cmp(&(b.0.to_lowercase(), b.1.to_lowercase()))
        });
        rows
    }

    /// The list as CSV: `Group,Command,Hotkeys`.
    pub fn export_csv(&self) -> String {
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("Group,Command,Hotkeys\n");
        for (g, c, k) in self.list_rows() {
            out.push_str(&format!("{},{},{}\n", quote(&g), quote(&c), quote(&k)));
        }
        out
    }

    /// The list as a printable two-column PDF (US Letter).
    pub fn list_pdf(&self) -> Vec<u8> {
        use plan_docs::pdf::PdfDoc;
        const W: f64 = 612.0;
        const H: f64 = 792.0;
        const MARGIN: f64 = 48.0;
        const GAP: f64 = 24.0;
        const ROW: f64 = 12.0;
        let col_w = (W - 2.0 * MARGIN - GAP) / 2.0;
        let rows = self.list_rows();
        let mut doc = PdfDoc::new(W, H);
        let top = H - MARGIN - 30.0;
        let bottom = MARGIN + 10.0;
        let header = |doc: &mut PdfDoc, page: usize| {
            doc.set_font_bold(true);
            doc.text(MARGIN, H - MARGIN - 8.0, 14.0, "Plan Studio hotkeys");
            doc.set_font_bold(false);
            doc.text(
                MARGIN,
                H - MARGIN - 22.0,
                8.0,
                &format!("{} commands with keys", rows.len()),
            );
            doc.text_right(W - MARGIN, MARGIN - 20.0, 8.0, &format!("Page {page}"));
        };
        let mut page = 1;
        header(&mut doc, page);
        let (mut col, mut y) = (0, top);
        let mut group = String::new();
        for (g, name, keys) in &rows {
            let needs_heading = *g != group;
            let need = ROW * if needs_heading { 3.0 } else { 1.0 };
            if y - need < bottom {
                if col == 0 {
                    col = 1;
                } else {
                    col = 0;
                    page += 1;
                    doc.new_page();
                    header(&mut doc, page);
                }
                y = top;
                if !needs_heading {
                    // A column that starts mid-group repeats the group name.
                    group.clear();
                }
            }
            let x = MARGIN + col as f64 * (col_w + GAP);
            if *g != group {
                y -= ROW * 0.6;
                doc.set_font_bold(true);
                doc.text(x, y - ROW, 9.5, g);
                doc.set_font_bold(false);
                doc.line(x, y - ROW - 2.5, x + col_w, y - ROW - 2.5, 0.4);
                y -= ROW * 1.4;
                group.clone_from(g);
            }
            y -= ROW;
            let keys_w = PdfDoc::text_width(keys, 8.5);
            let mut label = name.clone();
            while PdfDoc::text_width(&label, 8.5) > col_w - keys_w - 8.0 && label.len() > 4 {
                label.pop();
            }
            if label.len() < name.len() {
                label.push_str("..");
            }
            doc.text(x, y, 8.5, &label);
            doc.text_right(x + col_w, y, 8.5, keys);
        }
        doc.finish()
    }

    pub fn select(&mut self, name: &str) {
        self.selected = Some(name.to_string());
        self.chosen = None;
        self.conflict = None;
    }

    /// Adds a pressed chord to the sequence being recorded. Returns false
    /// when the sequence is already [`MAX_SEQUENCE`] long.
    pub fn record(&mut self, chord: Chord) -> bool {
        self.conflict = None;
        if self.seq.len() >= MAX_SEQUENCE {
            return false;
        }
        self.seq.push(chord);
        true
    }

    pub fn clear_sequence(&mut self) {
        self.seq.clear();
        self.conflict = None;
    }

    /// Assign: adds the recorded sequence to the selected command. With
    /// `steal` a clash is resolved by taking the sequence from the others.
    pub fn assign(&mut self, steal: bool) -> AssignResult {
        let Some(cmd) = self.selected.clone() else {
            return AssignResult::Nothing;
        };
        if self.seq.is_empty() {
            return AssignResult::Nothing;
        }
        match self.draft.assign(&cmd, self.seq.clone(), steal) {
            Ok(()) => {
                self.message = format!("{cmd}: {}", sequence_label(&self.seq));
                self.seq.clear();
                self.conflict = None;
                AssignResult::Assigned
            }
            Err(names) if names.is_empty() => AssignResult::Nothing,
            Err(names) => {
                self.conflict = Some(names.clone());
                AssignResult::Conflict(names)
            }
        }
    }

    /// Remove: drops the picked sequence of the selected command.
    pub fn remove_chosen(&mut self) -> bool {
        let (Some(cmd), Some(i)) = (self.selected.clone(), self.chosen) else {
            return false;
        };
        let Some(seq) = self.draft.sequences(&cmd).get(i).cloned() else {
            return false;
        };
        self.draft.remove(&cmd, &seq);
        self.chosen = None;
        true
    }

    pub fn choose(&mut self, index: usize) {
        self.chosen = Some(index);
    }

    /// Reset Hotkeys: Chief's defaults with Daniel's file over them again.
    pub fn reset(&mut self) {
        self.draft.reset();
        self.chosen = None;
        self.conflict = None;
        self.message = "Hotkeys reset to Chief's defaults and Daniel's file".into();
    }

    /// OK: copies the edited map into `target` and saves it to
    /// `~/.plan-studio/hotkeys.json`.
    pub fn commit(&self, target: &mut HotkeyMap) -> Result<(), String> {
        *target = self.draft.clone();
        target.save()
    }

    #[cfg(test)]
    pub fn commit_to(&self, target: &mut HotkeyMap, path: &std::path::Path) -> Result<(), String> {
        *target = self.draft.clone();
        target.save_to(path)
    }

    fn capture_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = e
            {
                if key == Key::Escape {
                    self.recording = false;
                } else {
                    self.record(Chord::from_event(key, &modifiers));
                }
            }
        }
    }

    /// Draws the window and reports OK / Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if self.recording {
            self.capture_keys(ctx);
        }
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Customize Hotkeys")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(560.0, 580.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                self.body(ui, &mut outcome);
            });
        if !open && outcome == Outcome::Open {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn body(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        ui.horizontal(|ui| {
            ui.label("Show Commands/Hotkeys Containing:");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter).desired_width(ui.available_width()),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Show:");
            for m in FilterMode::ALL {
                if ui
                    .selectable_label(self.filter_mode == m, m.label())
                    .clicked()
                {
                    self.filter_mode = m;
                }
            }
        });
        let conflicts = find_conflicts(&self.draft);
        let clashing: HashSet<&str> = conflicts
            .iter()
            .flat_map(|c| c.commands.iter().map(String::as_str))
            .collect();
        let s = self.draft.daniel_stats();
        ui.weak(format!(
            "Daniel's Chief hotkeys: {} named, {} have a Plan Studio command ({} work today).",
            s.named, s.mapped, s.live
        ));
        ui.add_space(4.0);

        let mut picked: Option<String> = None;
        let mut resolve: Option<(Owners, String)> = None;
        let folded = self.folded_collisions();
        egui::ScrollArea::vertical()
            .id_salt("hotkey_table")
            .max_height((ui.available_height() - 230.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let searching =
                    !self.filter.trim().is_empty() || self.filter_mode == FilterMode::Conflicts;
                if self.grouped {
                    for (menu, cmds) in self.grouped_commands() {
                        egui::CollapsingHeader::new(format!("{menu} ({})", cmds.len()))
                            .id_salt(("hotkey_menu", menu))
                            .default_open(searching || menu == "File")
                            .open(searching.then_some(true))
                            .show(ui, |ui| {
                                self.rows(ui, menu, &cmds, &clashing, &mut picked);
                            });
                    }
                } else {
                    let cmds = self.visible_commands();
                    self.rows(ui, "all", &cmds, &clashing, &mut picked);
                }
                if !conflicts.is_empty() {
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(
                        RichText::new(format!("Conflicts ({})", conflicts.len())).color(WARN),
                    )
                    .id_salt("hotkey_conflicts")
                    .default_open(true)
                    .show(ui, |ui| {
                        for c in &conflicts {
                            ui.label(format!("{}: {}", c.sequence, c.commands.join(", ")));
                            ui.horizontal_wrapped(|ui| {
                                for name in &c.commands {
                                    if ui
                                        .small_button(format!("Keep for {name}"))
                                        .on_hover_text("Takes the key from the other commands")
                                        .clicked()
                                    {
                                        resolve = Some((c.owners.clone(), name.clone()));
                                    }
                                }
                            });
                        }
                    });
                }
                if !folded.is_empty() {
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(
                        RichText::new(format!(
                            "Collide on Windows and Linux ({})",
                            folded.len()
                        ))
                        .color(WARN),
                    )
                    .id_salt("hotkey_folded")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.weak(
                            "There Control and Command are one key, so these differ here but not there.",
                        );
                        for c in &folded {
                            ui.label(format!(
                                "{}: {} ({})",
                                c.sequence,
                                c.commands.join(", "),
                                c.as_typed().join(" / ")
                            ));
                            ui.horizontal_wrapped(|ui| {
                                for name in &c.commands {
                                    if ui
                                        .small_button(format!("Keep for {name}"))
                                        .clicked()
                                    {
                                        resolve = Some((c.owners.clone(), name.clone()));
                                    }
                                }
                            });
                        }
                    });
                }
                let unmapped = self.draft.unmapped();
                if !unmapped.is_empty() {
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(format!(
                        "Chief bindings with no action in Plan Studio yet ({})",
                        unmapped.len()
                    ))
                    .id_salt("hotkey_unmapped")
                    .default_open(false)
                    .show(ui, |ui| {
                        egui::Grid::new("hotkey_unmapped_grid")
                            .num_columns(2)
                            .striped(true)
                            .spacing(Vec2::new(16.0, 3.0))
                            .show(ui, |ui| {
                                for u in unmapped {
                                    ui.weak(&u.name);
                                    ui.weak(&u.chord_text);
                                    ui.end_row();
                                }
                            });
                    });
                }
            });
        if let Some(name) = picked {
            self.select(&name);
        }
        if let Some((owners, keep)) = resolve {
            self.resolve(&owners, &keep);
        }

        ui.separator();
        self.assign_area(ui);

        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.grouped, "Group by menu");
            if self.is_default() {
                ui.weak("Chief's defaults with Daniel's file");
            } else {
                ui.weak("changed");
            }
            if ui
                .button("Reset Hotkeys")
                .on_hover_text("Chief's defaults with Daniel's UserHotkeys.xml over them")
                .clicked()
            {
                self.reset();
            }
            if ui
                .button("Reset to Chief Defaults")
                .on_hover_text("Chief's own default keys, without Daniel's file")
                .clicked()
            {
                self.reset_to_chief();
            }
            if ui
                .add_enabled(
                    !conflicts.is_empty(),
                    egui::Button::new("Resolve All Conflicts"),
                )
                .on_hover_text("Keeps each clashing key for the first command by name")
                .clicked()
            {
                self.resolve_all();
            }
            if ui
                .button("Import Chief Hotkeys\u{2026}")
                .on_hover_text("Reads a Chief UserHotkeys.xml over the keys above")
                .clicked()
            {
                self.import_file_dialog();
            }
            if ui
                .button("Export\u{2026}")
                .on_hover_text("UserHotkeys.xml (Chief's format), JSON or CSV")
                .clicked()
            {
                self.export_file_dialog();
            }
            if ui
                .button("Print List")
                .on_hover_text("Makes a PDF of the assigned keys and opens it to print")
                .clicked()
            {
                self.print_list();
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("OK").clicked() {
                *outcome = Outcome::Ok;
            }
            if ui.button("Cancel").clicked() {
                *outcome = Outcome::Cancel;
            }
            if ui.button("Help").clicked() {
                super::help::open(super::help::HOTKEYS_FILE);
                self.message =
                    "Pick a command, click the field, press up to 4 keys, then Assign.".into();
            }
        });
    }

    /// One menu's (or the flat) table of command, hotkey and group.
    fn rows(
        &self,
        ui: &mut egui::Ui,
        id: &str,
        cmds: &[&Command],
        clashing: &HashSet<&str>,
        picked: &mut Option<String>,
    ) {
        egui::Grid::new(("hotkey_grid", id))
            .num_columns(3)
            .striped(true)
            .spacing(Vec2::new(16.0, 3.0))
            .show(ui, |ui| {
                ui.strong("Command Name");
                ui.strong("Hotkey");
                ui.strong("Group");
                ui.end_row();
                for c in cmds {
                    let selected = self.selected.as_deref() == Some(c.name.as_str());
                    let text = if c.is_live() {
                        RichText::new(&c.name)
                    } else {
                        RichText::new(&c.name).weak()
                    };
                    let resp = ui.selectable_label(selected, text);
                    if resp.clicked() {
                        *picked = Some(c.name.clone());
                    }
                    if !c.is_live() {
                        resp.on_hover_text("Not built yet; the key is kept for later");
                    }
                    let keys = self.draft.hotkey_text(&c.name);
                    if clashing.contains(c.name.as_str()) {
                        ui.label(RichText::new(format!("{keys}  (conflict)")).color(WARN));
                    } else {
                        ui.label(keys);
                    }
                    ui.weak(&c.group);
                    ui.end_row();
                }
            });
    }

    fn import_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import Chief hotkeys")
            .add_filter("Chief hotkeys", &["xml"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                if let Err(e) = self.import_chief_xml(&text) {
                    self.message = format!("{} is not a hotkey file: {e}", path.display());
                }
            }
            Err(e) => self.message = format!("Could not read {}: {e}", path.display()),
        }
    }

    fn export_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export hotkeys")
            .set_file_name("UserHotkeys.xml")
            .add_filter("Chief hotkeys (UserHotkeys.xml)", &["xml"])
            .add_filter("Plan Studio hotkeys (JSON)", &["json"])
            .add_filter("Spreadsheet (CSV)", &["csv"])
            .save_file()
        else {
            return;
        };
        let ext = |e: &str| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e));
        let mut note = None;
        let text = if ext("csv") {
            self.export_csv()
        } else if ext("xml") {
            let (xml, stats) = self.export_chief_xml();
            note = Some(stats.summary());
            xml
        } else {
            self.export_json()
        };
        self.message = match std::fs::write(&path, text) {
            Ok(()) => match note {
                Some(n) => format!("Exported to {}. {n}", path.display()),
                None => format!("Exported to {}", path.display()),
            },
            Err(e) => format!("Could not write {}: {e}", path.display()),
        };
    }

    fn print_list(&mut self) {
        let path = std::env::temp_dir().join("plan-studio-hotkeys.pdf");
        self.message = match std::fs::write(&path, self.list_pdf()) {
            Ok(()) => match super::app_info::open_external(&path.to_string_lossy()) {
                Ok(()) => format!("Opened {}; print it from the viewer", path.display()),
                Err(e) => e,
            },
            Err(e) => format!("Could not write {}: {e}", path.display()),
        };
    }

    fn assign_area(&mut self, ui: &mut egui::Ui) {
        let Some(cmd) = self.selected.clone() else {
            ui.weak("Select a command to assign hotkeys.");
            if !self.message.is_empty() {
                ui.weak(&self.message);
            }
            return;
        };
        ui.label(format!(
            "Assign a sequence of up to {MAX_SEQUENCE} hotkeys to {cmd}:"
        ));
        let text = if self.seq.is_empty() {
            if self.recording {
                "Press keys...".to_string()
            } else {
                "Click here, then press keys".to_string()
            }
        } else {
            sequence_label(&self.seq)
        };
        let mut do_assign = false;
        let mut steal = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::Button::new(text)
                    .selected(self.recording)
                    .min_size(Vec2::new(260.0, 24.0)),
            );
            if field.clicked() {
                self.recording = true;
            } else if self.recording
                && ui.input(|i| i.pointer.any_pressed())
                && !field.contains_pointer()
            {
                self.recording = false;
            }
            if ui.button("Clear").clicked() {
                self.clear_sequence();
            }
            let can = !self.seq.is_empty();
            if ui.add_enabled(can, egui::Button::new("Assign")).clicked() {
                do_assign = true;
            }
        });
        // The clash shows as soon as the keys are recorded, before Assign.
        let pending = self.pending_conflicts();
        let shown = self
            .conflict
            .clone()
            .or((!pending.is_empty()).then_some(pending));
        if let Some(names) = &shown {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Already used by: {}.", names.join(", "))).color(WARN),
                );
                if ui.button("Reassign").clicked() {
                    steal = true;
                    do_assign = true;
                }
            });
        }
        if do_assign {
            self.assign(steal);
        }

        let current = self.draft.sequences(&cmd).to_vec();
        ui.horizontal_wrapped(|ui| {
            ui.label("Current hotkeys:");
            if current.is_empty() {
                ui.weak("none");
            }
            for (i, seq) in current.iter().enumerate() {
                if ui
                    .selectable_label(self.chosen == Some(i), sequence_label(seq))
                    .clicked()
                {
                    self.choose(i);
                }
            }
            if ui
                .add_enabled(self.chosen.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                self.remove_chosen();
            }
        });
        if !self.message.is_empty() {
            ui.weak(&self.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar::Action;

    fn chord(key: Key) -> Chord {
        Chord {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        }
    }

    fn dialog() -> HotkeyDialog {
        HotkeyDialog::new(&HotkeyMap::defaults())
    }

    #[test]
    fn filter_matches_command_names_and_hotkeys() {
        let mut d = dialog();
        let all = d.visible_commands().len();
        assert!(all > 100, "{all} commands");
        d.filter = "hinged".into();
        let hits = d.visible_commands();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Hinged Door");
        // By hotkey text.
        d.filter = "d, h".into();
        assert!(d.visible_commands().iter().any(|c| c.name == "Hinged Door"));
        d.filter = "zzzz".into();
        assert!(d.visible_commands().is_empty());
        d.filter = "".into();
        let names: Vec<_> = d
            .visible_commands()
            .iter()
            .map(|c| c.name.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn recording_stops_at_four_chords() {
        let mut d = dialog();
        for k in [Key::A, Key::B, Key::C, Key::D] {
            assert!(d.record(chord(k)));
        }
        assert!(!d.record(chord(Key::E)));
        assert_eq!(d.seq.len(), 4);
        d.clear_sequence();
        assert!(d.seq.is_empty());
    }

    #[test]
    fn assign_warns_on_conflict_and_reassign_moves_the_key() {
        let mut d = dialog();
        d.select("Zoom Out");
        d.record(chord(Key::Minus));
        assert_eq!(
            d.assign(false),
            AssignResult::Conflict(vec!["Zoom In".to_string()])
        );
        assert!(d.draft.sequences("Zoom Out").is_empty());
        assert_eq!(d.assign(true), AssignResult::Assigned);
        assert_eq!(d.draft.lookup(&[chord(Key::Minus)]), Some(Action::ZoomOut));
        assert!(d.draft.sequences("Zoom In").is_empty());
        // Nothing selected or nothing recorded.
        d.record(chord(Key::J));
        d.selected = None;
        assert_eq!(d.assign(false), AssignResult::Nothing);
    }

    #[test]
    fn remove_drops_the_chosen_sequence() {
        let mut d = dialog();
        d.select("Hinged Door");
        assert!(!d.remove_chosen(), "nothing chosen yet");
        d.choose(0);
        assert!(d.remove_chosen());
        // Only the number-key alias is left.
        assert_eq!(d.draft.hotkey_text("Hinged Door"), "3");
        d.reset();
        assert_eq!(d.draft.hotkey_text("Hinged Door"), "D, H; 3");
    }

    #[test]
    fn ok_writes_the_map_and_the_file() {
        let dir = std::env::temp_dir().join(format!("plan-studio-hkdlg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("hotkeys.json");
        let mut live = HotkeyMap::defaults();
        let mut d = HotkeyDialog::new(&live);
        d.select("Zoom Out");
        d.record(chord(Key::Equals));
        assert_eq!(d.assign(false), AssignResult::Assigned);
        // Not applied until OK.
        assert_eq!(live.lookup(&[chord(Key::Equals)]), None);
        d.commit_to(&mut live, &path).unwrap();
        assert_eq!(live.lookup(&[chord(Key::Equals)]), Some(Action::ZoomOut));
        let mut again = HotkeyMap::defaults();
        again.load_overrides(&path);
        assert_eq!(again.lookup(&[chord(Key::Equals)]), Some(Action::ZoomOut));
        let _ = std::fs::remove_dir_all(&dir);
    }
    fn with_prefix_clash() -> HotkeyMap {
        let mut map = HotkeyMap::defaults();
        // `D` alone now starts `D, H` (Hinged Door): a prefix clash that the
        // dialog's own Assign would have refused but a hand-edited file can make.
        let n = map.apply_overrides_json(r#"{"overrides": {"Zoom In": ["D"]}}"#);
        assert_eq!(n, 1);
        map
    }

    #[test]
    fn the_defaults_have_no_conflicts_and_a_clash_is_found_and_filtered() {
        assert!(find_conflicts(&HotkeyMap::defaults()).is_empty());
        let mut d = HotkeyDialog::new(&with_prefix_clash());
        let c = d.conflicts();
        assert!(
            c.iter().any(|c| c.sequence == "D"
                && c.commands.contains(&"Zoom In".to_string())
                && c.commands.contains(&"Hinged Door".to_string())),
            "{c:?}"
        );
        d.set_filter_mode(FilterMode::Conflicts);
        let shown: Vec<&str> = d
            .visible_commands()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(
            shown.contains(&"Zoom In") && shown.contains(&"Hinged Door"),
            "{shown:?}"
        );
        assert!(shown.len() < 20);
        d.set_filter_mode(FilterMode::Unassigned);
        assert!(d
            .visible_commands()
            .iter()
            .all(|c| d.draft.sequences(&c.name).is_empty()));
        d.set_filter_mode(FilterMode::Assigned);
        assert!(d
            .visible_commands()
            .iter()
            .all(|c| !d.draft.sequences(&c.name).is_empty()));
    }

    #[test]
    fn a_recorded_sequence_shows_its_clash_before_assign() {
        let mut d = dialog();
        d.select("Zoom Out");
        assert!(d.pending_conflicts().is_empty());
        d.record(chord(Key::Minus));
        assert_eq!(d.pending_conflicts(), vec!["Zoom In".to_string()]);
        d.clear_sequence();
        assert!(d.pending_conflicts().is_empty());
    }

    #[test]
    fn search_matches_the_group_too() {
        let mut d = dialog();
        d.set_filter("straight wall");
        let names: Vec<&str> = d
            .visible_commands()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(names.len() >= 5, "{names:?}");
        assert!(names.contains(&"Straight Interior Wall"));
    }

    #[test]
    fn importing_chief_hotkeys_again_restores_daniels_keys_over_edits() {
        const XML: &str = include_str!("../../../../docs/chief-config-raw/UserHotkeys.xml");
        let mut d = dialog();
        d.select("Hinged Door");
        d.choose(0);
        assert!(d.remove_chosen());
        d.select("Zoom Out");
        d.record(chord(Key::Equals));
        assert_eq!(d.assign(false), AssignResult::Assigned);
        assert_eq!(d.draft.hotkey_text("Hinged Door"), "3");
        let r = d.import_chief_xml(XML).unwrap();
        assert!(r.bindings >= 120, "{r:?}");
        assert!(r.commands >= 100 && r.commands <= r.bindings, "{r:?}");
        assert!(r.summary().starts_with("Imported"));
        // Hinged Door is back on Daniel's D, H, and Zoom In on his `-`
        // (Chief binds it there).
        assert!(d.draft.hotkey_text("Hinged Door").starts_with("D, H"));
        assert_eq!(d.draft.hotkey_text("Zoom In"), "-");
        assert!(d.conflicts().is_empty(), "{:?}", d.conflicts());
        assert!(d.import_chief_xml("not xml <").is_err());
    }

    #[test]
    fn export_and_print_list_the_assigned_keys() {
        let d = dialog();
        let rows = d.list_rows();
        assert!(rows.len() > 100);
        assert!(rows
            .iter()
            .any(|(_, n, k)| n == "Hinged Door" && k.starts_with("D, H")));
        let mut sorted = rows.clone();
        sorted.sort_by(|a, b| {
            (a.0.to_lowercase(), a.1.to_lowercase()).cmp(&(b.0.to_lowercase(), b.1.to_lowercase()))
        });
        assert_eq!(rows, sorted, "by group then name");
        let csv = d.export_csv();
        assert!(csv.starts_with("Group,Command,Hotkeys\n"));
        assert!(csv.contains("\"Hinged Door\",\"D, H"));
        assert_eq!(csv.lines().count(), rows.len() + 1);
        // JSON: only the differences from the defaults.
        assert!(d.export_json().contains("\"overrides\": {}"));
        let pdf = d.list_pdf();
        assert!(pdf.starts_with(b"%PDF"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("Plan Studio hotkeys") && text.contains("Hinged Door"));
        assert!(pdf.len() > 3_000);
    }

    #[test]
    fn the_window_draws_headlessly_with_a_conflict_and_a_selection() {
        let mut d = HotkeyDialog::new(&with_prefix_clash());
        d.select("Zoom In");
        let ctx = egui::Context::default();
        let mut outcome = Outcome::Open;
        let _ = ctx.run(egui::RawInput::default(), |ctx| outcome = d.show(ctx));
        assert_eq!(outcome, Outcome::Open);
    }
}
