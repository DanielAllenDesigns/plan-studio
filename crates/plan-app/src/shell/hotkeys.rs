//! The runtime hotkey map.
//!
//! The fixed [`toolbar::BINDINGS`] table is only the *base*: it is turned into
//! a [`HotkeyMap`] keyed by command name, then overridden by Daniel's Chief
//! hotkeys (`plan_config::load_daniel_config`), then by the user's own edits in
//! `~/.plan-studio/hotkeys.json` (written by the Customize Hotkeys dialog).
//!
//! * A **command** is anything Plan Studio can do and name: every toolbar
//!   button and flyout entry, plus a few extras ([`collect_commands`]). A
//!   command that is still `Action::NotImplemented` counts as known but not
//!   live (pressing its key reports "Not yet implemented").
//! * A **sequence** is one to four [`Chord`]s (`D, H`). A command may have
//!   several sequences.
//! * Qt-to-Mac modifier swap: `plan-config` already stores the physical Mac
//!   meaning (`ctrl` = Control, `meta` = Command); [`Chord`] keeps it. Where
//!   there is no Command key (Windows, Linux) Chief's "Ctrl" (= Command)
//!   becomes the Control key and the Mac Control modifier folds into it
//!   ([`platform_modifiers`]), so `meta` then stands for the Control key.
//!
//! [`handle`] is the per-frame entry point used by `main.rs`; it keeps the
//! 1.5 s multi-key buffer.

use crate::editor::EditorContext;
use crate::shell::view3d_panel::View3dCommand;
use crate::toolbar::{self, Action, Binding, Slot, Toolbars, ViewFlag, BINDINGS, SEQUENCE_TIMEOUT};
use eframe::egui::{self, Key, Modifiers};
use plan_config::KeyChord;
use plan_config::{format_sequence, load_daniel_config, normalize_key, to_plan_studio_bindings};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The longest key sequence Chief (and the dialog) allows.
pub const MAX_SEQUENCE: usize = 4;

/// The modifiers a chord keeps on a platform. macOS has both Control and
/// Command, so they stay as they are. Elsewhere there is no Command key:
/// Chief's "Ctrl" (the `meta` flag) is the Control key and the Mac Control
/// flag has no key of its own, so both fold into `meta` and `ctrl` is never
/// set. Returns `(ctrl, meta)`.
pub fn platform_modifiers(ctrl: bool, meta: bool, mac: bool) -> (bool, bool) {
    if mac {
        (ctrl, meta)
    } else {
        (false, ctrl || meta)
    }
}

/// `Cmd+` in a chord label is `Ctrl+` where there is no Command key.
pub fn label_for_platform(text: &str, mac: bool) -> String {
    if mac {
        text.to_string()
    } else {
        text.replace("Cmd+", "Ctrl+")
    }
}

/// One key press with modifiers: `ctrl` is the Control key, `meta` the
/// Command key (the Control key stands in for it on other platforms).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: Key,
}

impl Chord {
    /// The chord an egui key press stands for.
    pub fn from_event(key: Key, m: &Modifiers) -> Chord {
        // egui's `command` is the Command key on macOS and Control elsewhere.
        let (ctrl, meta) = if cfg!(target_os = "macos") {
            (m.ctrl, m.mac_cmd)
        } else {
            platform_modifiers(m.ctrl, m.command, false)
        };
        Chord {
            ctrl,
            alt: m.alt,
            shift: m.shift,
            meta,
            key,
        }
    }

    /// From `plan-config`'s chord; `None` when the key has no egui key.
    pub fn from_config(c: &KeyChord) -> Option<Chord> {
        let (key, implied_shift) = key_from_name(&c.key)?;
        let (ctrl, meta) = platform_modifiers(c.ctrl, c.meta, cfg!(target_os = "macos"));
        Some(Chord {
            ctrl,
            alt: c.alt,
            shift: c.shift || implied_shift,
            meta,
            key,
        })
    }

    /// The same chord with its modifiers folded to this platform's
    /// convention (Control and Command are one key off the Mac), so a chord
    /// assigned from a test or a file matches one captured from the keyboard.
    pub fn normalized(self) -> Chord {
        let (ctrl, meta) = platform_modifiers(self.ctrl, self.meta, cfg!(target_os = "macos"));
        Chord { ctrl, meta, ..self }
    }

    pub fn to_config(self) -> KeyChord {
        KeyChord {
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            meta: self.meta,
            key: key_name(self.key),
        }
    }
}

/// `D, H` style text for a sequence.
pub fn sequence_label(seq: &[Chord]) -> String {
    let keys: Vec<KeyChord> = seq.iter().copied().map(Chord::to_config).collect();
    label_for_platform(&format_sequence(&keys), cfg!(target_os = "macos"))
}

fn key_name(k: Key) -> String {
    let punct = match k {
        Key::Minus => "-",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::Backtick => "`",
        Key::Backslash => "\\",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::OpenCurlyBracket => "{",
        Key::CloseCurlyBracket => "}",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::Colon => ":",
        Key::Pipe => "|",
        Key::Questionmark => "?",
        Key::Exclamationmark => "!",
        _ => return normalize_key(k.name()),
    };
    punct.to_string()
}

/// The egui key for a normalized key name; the flag is true when the name is
/// a shifted symbol (`~` is Shift+Backtick).
fn key_from_name(name: &str) -> Option<(Key, bool)> {
    match name {
        "~" => Some((Key::Backtick, true)),
        "Num+" => Some((Key::Plus, false)),
        "Num-" => Some((Key::Minus, false)),
        "Del" => Some((Key::Delete, false)),
        other => Key::from_name(other).map(|k| (k, false)),
    }
}

// ----- commands -----

/// A named thing Plan Studio can do.
#[derive(Clone, Debug)]
pub struct Command {
    pub name: String,
    pub group: String,
    pub action: Action,
}

impl Command {
    /// False for toolbar entries that are not built yet.
    pub fn is_live(&self) -> bool {
        !matches!(self.action, Action::NotImplemented(_))
    }
}

/// Chief names Daniel's file uses that Plan Studio files under another name.
const ALIASES: &[(&str, &str)] = &[
    ("New Project", "New Plan"),
    ("Open Plan/Layout", "Open Plan"),
];

/// Commands that have no toolbar button of their own.
fn extra_commands() -> Vec<Command> {
    let c = |group: &str, name: &str, action| Command {
        name: name.to_string(),
        group: group.to_string(),
        action,
    };
    vec![
        c("File", "Save As", Action::FileSaveAs),
        c("File", "Quit", Action::Quit),
        c(
            "File",
            "Save Current Defaults as My Template",
            Action::SaveTemplate,
        ),
        c("File", "Reset to Chief X18 Template", Action::ResetTemplate),
        c(
            "View",
            "Reference Grid",
            Action::ToggleFlag(ViewFlag::ReferenceGrid),
        ),
        c(
            "3D",
            "3D View Defaults",
            Action::View3d(View3dCommand::Defaults),
        ),
        c("Tools", "Current Wall Tool", Action::CurrentWall),
        c("Tools", "Customize Hotkeys", Action::OpenHotkeyDialog),
        c("Tools", "Layer Display Options", Action::OpenLayerDisplay),
        c("Help", "About Plan Studio", Action::ShowAbout),
    ]
}

/// Every command Plan Studio has, read from the toolbar and menu tables so a
/// tool that stops being `NotImplemented` becomes live here without any
/// change. A name seen twice keeps the live action.
pub fn collect_commands() -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    let mut add = |group: &str, name: &str, action: Action| {
        if let Some(existing) = out.iter_mut().find(|c| c.name == name) {
            if !existing.is_live() && !matches!(action, Action::NotImplemented(_)) {
                existing.action = action;
            }
            return;
        }
        out.push(Command {
            name: name.to_string(),
            group: group.to_string(),
            action,
        });
    };
    let bars = Toolbars::new();
    for (bar, slots) in [
        ("File and Edit", &bars.row1),
        ("Build and CAD", &bars.row2),
        ("View", &bars.view),
    ] {
        for slot in slots {
            match slot {
                Slot::Button(it) | Slot::Toggle(it) => add(bar, it.name, it.action),
                Slot::Flyout(f) => {
                    for e in &f.entries {
                        add(f.group, e.name, e.action);
                    }
                }
                _ => {}
            }
        }
    }
    let mut flyouts = Vec::new();
    for g in toolbar::build_menu() {
        flyouts.extend(g.flyouts);
    }
    flyouts.extend(toolbar::terrain_menu());
    flyouts.extend([
        toolbar::cad_blocks(),
        toolbar::points(),
        toolbar::lines(),
        toolbar::arcs(),
        toolbar::circles(),
        toolbar::boxes(),
        toolbar::dimensions(),
        toolbar::auto_dimensions(),
        toolbar::text_tools(),
    ]);
    for f in &flyouts {
        for e in &f.entries {
            add(f.group, e.name, e.action);
        }
    }
    for c in extra_commands() {
        add(&c.group, &c.name, c.action);
    }
    out
}

fn canonical(name: &str) -> &str {
    ALIASES
        .iter()
        .find(|(from, _)| *from == name)
        .map_or(name, |(_, to)| to)
}

// ----- the map -----

/// A Chief binding of Daniel's whose command Plan Studio does not have yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnmappedBinding {
    pub name: String,
    pub chord_text: String,
}

/// How Daniel's named Chief bindings landed in Plan Studio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DanielStats {
    /// Bindings that carry a resolved command name.
    pub named: usize,
    /// Of those, commands Plan Studio knows (live or not).
    pub mapped: usize,
    /// Of the mapped, commands that do something today.
    pub live: usize,
    /// Named but with no Plan Studio command yet.
    pub unmapped: usize,
}

type Bindings = BTreeMap<String, Vec<Vec<Chord>>>;

#[derive(Clone, Debug)]
pub struct HotkeyMap {
    commands: Vec<Command>,
    bindings: Bindings,
    defaults: Bindings,
    index: HashMap<Vec<Chord>, Action>,
    prefixes: HashSet<Vec<Chord>>,
    unmapped: Vec<UnmappedBinding>,
    stats: DanielStats,
}

/// The chord sequence a `toolbar::Binding` stands for.
fn binding_sequence(b: &Binding) -> Vec<Chord> {
    let (ctrl, meta) = platform_modifiers(b.ctrl, b.command, cfg!(target_os = "macos"));
    b.keys
        .iter()
        .map(|k| Chord {
            ctrl,
            alt: false,
            shift: b.shift,
            meta,
            key: *k,
        })
        .collect()
}

/// Ctrl+Option+Cmd+L (Adjust Lights) as this platform keeps it: off the Mac
/// the Control and Command flags fold into the one Control key, so the chord
/// is Ctrl+Alt+L there.
fn adjust_lights_chord(mac: bool) -> Chord {
    let (ctrl, meta) = platform_modifiers(true, true, mac);
    Chord {
        ctrl,
        alt: true,
        shift: false,
        meta,
        key: Key::L,
    }
}

/// Chief defaults that need the Option (alt) key, which a
/// [`toolbar::Binding`] cannot express: command name and sequence.
fn alt_defaults() -> Vec<(&'static str, Vec<Chord>)> {
    vec![(
        "Adjust Lights",
        vec![adjust_lights_chord(cfg!(target_os = "macos"))],
    )]
}

/// Plan Studio's own extras (number keys, Shift+Cmd+Z) survive Daniel's
/// overrides as long as his chords do not take them.
fn is_plan_studio_alias(b: &Binding) -> bool {
    let number = matches!(b.keys, [Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4])
        && !b.shift
        && !b.ctrl
        && !b.command;
    let redo = b.keys == [Key::Z] && b.shift && b.command;
    number || redo
}

impl HotkeyMap {
    /// Chief's defaults overridden by Daniel's bindings; nothing from disk.
    pub fn defaults() -> HotkeyMap {
        let commands = collect_commands();
        let mut bindings: Bindings = BTreeMap::new();
        let name_of = |action: Action| {
            commands
                .iter()
                .find(|c| c.action == action)
                .map(|c| c.name.clone())
        };
        for b in BINDINGS {
            if let Some(name) = name_of(b.action) {
                push_unique(bindings.entry(name).or_default(), binding_sequence(b));
            }
        }
        for (name, seq) in alt_defaults() {
            if commands.iter().any(|c| c.name == name) {
                push_unique(bindings.entry(name.to_string()).or_default(), seq);
            }
        }

        // Daniel's bindings.
        let cfg = load_daniel_config();
        let mut daniel: Bindings = BTreeMap::new();
        let mut unmapped = Vec::new();
        let mut stats = DanielStats::default();
        for pb in to_plan_studio_bindings(&cfg.hotkeys) {
            stats.named += 1;
            let name = canonical(&pb.command_name).to_string();
            let seq: Option<Vec<Chord>> = pb.keys.iter().map(Chord::from_config).collect();
            match (commands.iter().find(|c| c.name == name), seq) {
                (Some(cmd), Some(seq)) => {
                    stats.mapped += 1;
                    if cmd.is_live() {
                        stats.live += 1;
                    }
                    push_unique(daniel.entry(name).or_default(), seq);
                }
                _ => {
                    stats.unmapped += 1;
                    unmapped.push(UnmappedBinding {
                        name: pb.command_name,
                        chord_text: pb.chord_text,
                    });
                }
            }
        }
        // Daniel's sequences take their chords away from the base.
        for seqs in daniel.values() {
            for seq in seqs {
                for list in bindings.values_mut() {
                    list.retain(|s| s != seq);
                }
            }
        }
        for (name, seqs) in daniel {
            bindings.insert(name, seqs);
        }
        // Plan Studio's own extras come back unless a chord is now taken.
        for b in BINDINGS.iter().filter(|b| is_plan_studio_alias(b)) {
            let seq = binding_sequence(b);
            let taken = bindings.values().flatten().any(|s| *s == seq);
            if let (false, Some(name)) = (taken, name_of(b.action)) {
                push_unique(bindings.entry(name).or_default(), seq);
            }
        }
        bindings.retain(|_, v| !v.is_empty());

        let mut map = HotkeyMap {
            commands,
            defaults: bindings.clone(),
            bindings,
            index: HashMap::new(),
            prefixes: HashSet::new(),
            unmapped,
            stats,
        };
        map.rebuild();
        map
    }

    /// The defaults plus the user's overrides from `~/.plan-studio/hotkeys.json`.
    pub fn load() -> HotkeyMap {
        let mut map = HotkeyMap::defaults();
        if let Some(path) = user_path() {
            map.load_overrides(&path);
        }
        map
    }

    /// Applies the overrides saved at `path` (missing or unreadable files are
    /// ignored). Returns how many commands were overridden.
    pub fn load_overrides(&mut self, path: &Path) -> usize {
        let Ok(text) = std::fs::read_to_string(path) else {
            return 0;
        };
        self.apply_overrides_json(&text)
    }

    pub fn apply_overrides_json(&mut self, text: &str) -> usize {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return 0;
        };
        let Some(map) = value.get("overrides").and_then(|v| v.as_object()) else {
            return 0;
        };
        let mut applied = 0;
        for (name, list) in map {
            if !self.commands.iter().any(|c| c.name == *name) {
                continue;
            }
            let Some(list) = list.as_array() else {
                continue;
            };
            let mut seqs: Vec<Vec<Chord>> = Vec::new();
            for text in list.iter().filter_map(|v| v.as_str()) {
                let parsed = KeyChord::parse_file_sequence(text).ok().and_then(|ks| {
                    ks.iter()
                        .map(Chord::from_config)
                        .collect::<Option<Vec<_>>>()
                });
                if let Some(seq) = parsed.filter(|s| !s.is_empty() && s.len() <= MAX_SEQUENCE) {
                    push_unique(&mut seqs, seq);
                }
            }
            for seq in &seqs {
                for (other, list) in self.bindings.iter_mut() {
                    if other != name {
                        list.retain(|s| s != seq);
                    }
                }
            }
            if seqs.is_empty() {
                self.bindings.remove(name);
            } else {
                self.bindings.insert(name.clone(), seqs);
            }
            applied += 1;
        }
        self.rebuild();
        applied
    }

    /// The overrides as JSON: only commands whose sequences differ from the
    /// defaults. An empty list means "unbound".
    pub fn overrides_json(&self) -> String {
        let mut over = serde_json::Map::new();
        let names: HashSet<&String> = self.bindings.keys().chain(self.defaults.keys()).collect();
        for name in names {
            let now = self.bindings.get(name).cloned().unwrap_or_default();
            let was = self.defaults.get(name).cloned().unwrap_or_default();
            if now != was {
                let texts: Vec<serde_json::Value> = now
                    .iter()
                    .map(|seq| {
                        let keys: Vec<String> =
                            seq.iter().map(|c| c.to_config().to_file_string()).collect();
                        serde_json::Value::String(keys.join(", "))
                    })
                    .collect();
                over.insert(name.clone(), serde_json::Value::Array(texts));
            }
        }
        let mut sorted: Vec<(String, serde_json::Value)> = over.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let overrides: serde_json::Map<String, serde_json::Value> = sorted.into_iter().collect();
        let root = serde_json::json!({ "version": 1, "overrides": overrides });
        serde_json::to_string_pretty(&root).unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, self.overrides_json()).map_err(|e| e.to_string())
    }

    /// Writes `~/.plan-studio/hotkeys.json`.
    pub fn save(&self) -> Result<(), String> {
        let path = user_path().ok_or(crate::paths::NO_HOME)?;
        self.save_to(&path)
    }

    fn rebuild(&mut self) {
        self.index.clear();
        self.prefixes.clear();
        for c in &self.commands {
            for seq in self.bindings.get(&c.name).into_iter().flatten() {
                self.index.entry(seq.clone()).or_insert(c.action);
                for n in 1..seq.len() {
                    self.prefixes.insert(seq[..n].to_vec());
                }
            }
        }
    }

    // ----- queries -----

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn command(&self, name: &str) -> Option<&Command> {
        let name = canonical(name);
        self.commands.iter().find(|c| c.name == name)
    }

    /// The sequences bound to `name`.
    pub fn sequences(&self, name: &str) -> &[Vec<Chord>] {
        self.bindings.get(name).map_or(&[], Vec::as_slice)
    }

    /// `D, H; Cmd+Y` style text for a command's hotkeys.
    pub fn hotkey_text(&self, name: &str) -> String {
        self.sequences(name)
            .iter()
            .map(|s| sequence_label(s))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// The action a complete sequence triggers.
    pub fn lookup(&self, seq: &[Chord]) -> Option<Action> {
        self.index.get(seq).copied()
    }

    /// Does a longer binding start with `seq`?
    pub fn is_prefix(&self, seq: &[Chord]) -> bool {
        self.prefixes.contains(seq)
    }

    /// Daniel's named bindings whose command Plan Studio does not have yet.
    pub fn unmapped(&self) -> &[UnmappedBinding] {
        &self.unmapped
    }

    pub fn daniel_stats(&self) -> DanielStats {
        self.stats
    }

    // ----- editing -----

    /// Commands (other than `cmd`) whose hotkeys clash with `seq`: the same
    /// sequence, or one that is a prefix of the other (`D` vs `D, H`).
    pub fn conflicts(&self, cmd: &str, seq: &[Chord]) -> Vec<String> {
        let mut out = Vec::new();
        for (name, list) in &self.bindings {
            if name == cmd {
                continue;
            }
            let clash = list
                .iter()
                .any(|s| s.as_slice() == seq || s.starts_with(seq) || seq.starts_with(s));
            if clash {
                out.push(name.clone());
            }
        }
        out
    }

    /// Adds `seq` to `cmd`. With a conflict it returns the clashing command
    /// names and changes nothing, unless `steal` removes the clashing
    /// sequences from the other commands first.
    pub fn assign(&mut self, cmd: &str, seq: Vec<Chord>, steal: bool) -> Result<(), Vec<String>> {
        let seq: Vec<Chord> = seq.into_iter().map(Chord::normalized).collect();
        if seq.is_empty() || seq.len() > MAX_SEQUENCE || self.command(cmd).is_none() {
            return Err(Vec::new());
        }
        let clashes = self.conflicts(cmd, &seq);
        if !clashes.is_empty() {
            if !steal {
                return Err(clashes);
            }
            for name in &clashes {
                if let Some(list) = self.bindings.get_mut(name) {
                    list.retain(|s| !(s.starts_with(&seq) || seq.starts_with(s)));
                }
            }
            self.bindings.retain(|_, v| !v.is_empty());
        }
        push_unique(self.bindings.entry(cmd.to_string()).or_default(), seq);
        self.rebuild();
        Ok(())
    }

    /// Removes one sequence of `cmd`.
    pub fn remove(&mut self, cmd: &str, seq: &[Chord]) {
        if let Some(list) = self.bindings.get_mut(cmd) {
            list.retain(|s| s.as_slice() != seq);
            if list.is_empty() {
                self.bindings.remove(cmd);
            }
        }
        self.rebuild();
    }

    /// Back to Chief's and Daniel's defaults, dropping every user edit.
    pub fn reset(&mut self) {
        self.bindings = self.defaults.clone();
        self.rebuild();
    }
}

fn push_unique(list: &mut Vec<Vec<Chord>>, seq: Vec<Chord>) {
    if !list.contains(&seq) {
        list.push(seq);
    }
}

/// `~/.plan-studio/hotkeys.json`, or `None` when no home directory is known.
pub fn user_path() -> Option<PathBuf> {
    crate::paths::user_file("hotkeys.json")
}

// ----- the per-frame state machine -----

/// The map plus the multi-key buffer.
pub struct HotkeyState {
    pub map: HotkeyMap,
    pending: Vec<Chord>,
    since: Option<Instant>,
    /// Set by a modal dialog every frame it is open; [`handle`] then ignores
    /// the keyboard (the dialog is recording a chord).
    pub suspended: bool,
}

impl Default for HotkeyState {
    fn default() -> Self {
        HotkeyState::new(HotkeyMap::load())
    }
}

impl HotkeyState {
    pub fn new(map: HotkeyMap) -> HotkeyState {
        HotkeyState {
            map,
            pending: Vec::new(),
            since: None,
            suspended: false,
        }
    }

    /// The pending sequence prefix for the status bar, like `"D, ..."`.
    pub fn pending_label(&self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        Some(format!("{}, ...", sequence_label(&self.pending)))
    }

    fn clear(&mut self) {
        self.pending.clear();
        self.since = None;
    }

    fn expire(&mut self, now: Instant) {
        if self
            .since
            .is_some_and(|t| now.saturating_duration_since(t) >= SEQUENCE_TIMEOUT)
        {
            self.clear();
        }
    }

    /// Feeds one key press; triggered actions are appended to `out`.
    pub fn press(&mut self, chord: Chord, now: Instant, out: &mut Vec<Action>) {
        if chord.key == Key::Escape {
            self.clear();
            return;
        }
        self.expire(now);
        if !self.pending.is_empty() {
            let mut seq = self.pending.clone();
            seq.push(chord);
            if let Some(a) = self.map.lookup(&seq) {
                out.push(a);
                self.clear();
                return;
            }
            if seq.len() < MAX_SEQUENCE && self.map.is_prefix(&seq) {
                self.pending = seq;
                self.since = Some(now);
                return;
            }
            // Not a continuation: drop the prefix and treat this as a fresh press.
            self.clear();
        }
        let single = [chord];
        if let Some(a) = self.map.lookup(&single) {
            out.push(a);
        } else if self.map.is_prefix(&single) {
            self.pending = vec![chord];
            self.since = Some(now);
        }
    }
}

/// Reads this frame's key presses (nothing while a text field has keyboard
/// focus or a dialog is recording) and appends the actions they trigger.
pub fn handle(
    ctx: &egui::Context,
    cx: &EditorContext,
    state: &mut HotkeyState,
    out: &mut Vec<Action>,
) {
    let suspended = std::mem::take(&mut state.suspended);
    state.expire(Instant::now());
    if suspended || cx.temp.editing.is_some() || ctx.wants_keyboard_input() {
        state.clear();
        return;
    }
    let presses: Vec<Chord> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some(Chord::from_event(*key, modifiers)),
                _ => None,
            })
            .collect()
    });
    let now = Instant::now();
    for chord in presses {
        state.press(chord, now, out);
    }
    if !state.pending.is_empty() {
        ctx.request_repaint_after(SEQUENCE_TIMEOUT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolId;
    use std::time::Duration;

    #[test]
    fn modifiers_fold_without_a_command_key() {
        // macOS keeps both.
        assert_eq!(platform_modifiers(true, true, true), (true, true));
        assert_eq!(platform_modifiers(false, true, true), (false, true));
        // Elsewhere Chief's Ctrl (= Command) is the Control key, and the Mac
        // Control flag folds into it.
        assert_eq!(platform_modifiers(false, true, false), (false, true));
        assert_eq!(platform_modifiers(true, false, false), (false, true));
        assert_eq!(platform_modifiers(true, true, false), (false, true));
        assert_eq!(platform_modifiers(false, false, false), (false, false));
        assert_eq!(
            label_for_platform("Cmd+S, Shift+Cmd+Z", false),
            "Ctrl+S, Shift+Ctrl+Z"
        );
        assert_eq!(label_for_platform("Cmd+S", true), "Cmd+S");
    }

    fn plain(key: Key) -> Chord {
        Chord {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        }
    }

    fn cmd_chord(key: Key) -> Chord {
        Chord {
            meta: true,
            ..plain(key)
        }
    }

    fn temp_file(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("plan-studio-hotkeys-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("hotkeys.json")
    }

    #[test]
    fn daniels_minus_is_zoom_in_and_d_h_is_hinged_door() {
        let map = HotkeyMap::defaults();
        assert_eq!(map.lookup(&[plain(Key::Minus)]), Some(Action::ZoomIn));
        let dh = [plain(Key::D), plain(Key::H)];
        assert_eq!(map.lookup(&dh), Some(Action::SetTool(ToolId::Door)));
        assert!(map.is_prefix(&[plain(Key::D)]));
        // Plan Studio's own number-key alias rides along.
        assert_eq!(map.hotkey_text("Hinged Door"), "D, H; 3");
        assert_eq!(map.hotkey_text("Zoom In"), "-");
        // Daniel's modifiers: Control+Z is Down One Floor, Command+Z is Undo.
        let ctrl_z = Chord {
            ctrl: true,
            ..plain(Key::Z)
        };
        if cfg!(target_os = "macos") {
            assert_eq!(map.lookup(&[ctrl_z]), Some(Action::FloorDown));
            assert_eq!(map.lookup(&[cmd_chord(Key::Z)]), Some(Action::Undo));
        } else {
            // One Ctrl key off the Mac: Control+Z and Command+Z collapse onto
            // Ctrl+Z, and whichever Daniel's file lists wins; it must bind.
            assert!(map.lookup(&[ctrl_z.normalized()]).is_some());
            assert!(map.lookup(&[cmd_chord(Key::Z)]).is_some());
        }
        // Plan Studio's own number keys survive.
        assert_eq!(
            map.lookup(&[plain(Key::Num1)]),
            Some(Action::SetTool(ToolId::Select))
        );
    }

    #[test]
    fn adjust_lights_is_bound_to_ctrl_option_cmd_l() {
        let map = HotkeyMap::defaults();
        let cmd = map.command("Adjust Lights").expect("the command exists");
        assert_eq!(cmd.action, Action::View3d(View3dCommand::AdjustLights));
        assert!(cmd.is_live());
        let chord = adjust_lights_chord(cfg!(target_os = "macos"));
        assert_eq!(map.lookup(&[chord]), Some(cmd.action));
        assert!(map.sequences("Adjust Lights").contains(&vec![chord]));
        // The chord is the same one a key press builds.
        let pressed = if cfg!(target_os = "macos") {
            Modifiers {
                ctrl: true,
                alt: true,
                mac_cmd: true,
                command: true,
                ..Modifiers::NONE
            }
        } else {
            Modifiers {
                ctrl: true,
                alt: true,
                command: true,
                ..Modifiers::NONE
            }
        };
        assert_eq!(Chord::from_event(Key::L, &pressed), chord);
        // Mac keeps Control and Command apart; elsewhere they fold into one
        // Control key (shown as Ctrl).
        let mac = adjust_lights_chord(true);
        assert!(mac.ctrl && mac.meta && mac.alt);
        let other = adjust_lights_chord(false);
        assert!(!other.ctrl && other.meta && other.alt);
        assert_eq!(other.normalized(), other);
        assert_eq!(
            label_for_platform(&sequence_label(&[mac]), true)
                .matches('+')
                .count(),
            3
        );
        // A user override can still take the chord.
        let mut map = map;
        map.assign("Zoom In", vec![chord], true).unwrap();
        assert_eq!(map.lookup(&[chord]), Some(Action::ZoomIn));
        assert!(map.sequences("Adjust Lights").is_empty());
    }

    #[test]
    fn daniels_named_bindings_are_counted() {
        let map = HotkeyMap::defaults();
        let s = map.daniel_stats();
        eprintln!("daniel stats: {s:?}");
        eprintln!(
            "unmapped: {:?}",
            map.unmapped()
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(s.named, 143);
        assert_eq!(s.mapped + s.unmapped, s.named);
        assert!(s.live > 15, "live mapped: {}", s.live);
        assert_eq!(map.unmapped().len(), s.unmapped);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn egui_modifiers_map_to_the_mac_convention() {
        let m = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(Chord::from_event(Key::Z, &m), cmd_chord(Key::Z));
        let c = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        assert!(Chord::from_event(Key::F, &c).ctrl);
    }

    #[test]
    fn sequences_use_the_buffer_and_expire() {
        let mut st = HotkeyState::new(HotkeyMap::defaults());
        let t = Instant::now();
        let mut out = Vec::new();
        st.press(plain(Key::D), t, &mut out);
        assert!(out.is_empty());
        assert_eq!(st.pending_label().as_deref(), Some("D, ..."));
        st.press(plain(Key::H), t + Duration::from_millis(100), &mut out);
        assert_eq!(out, vec![Action::SetTool(ToolId::Door)]);
        assert!(st.pending_label().is_none());

        // The prefix times out after 1.5 s: H alone is then Pan Window.
        out.clear();
        st.press(plain(Key::D), t, &mut out);
        st.press(
            plain(Key::H),
            t + SEQUENCE_TIMEOUT + Duration::from_millis(1),
            &mut out,
        );
        assert_eq!(out, vec![Action::TogglePan]);

        // A key that does not continue the sequence starts afresh.
        out.clear();
        st.press(plain(Key::D), t, &mut out);
        st.press(plain(Key::Space), t, &mut out);
        assert_eq!(out, vec![Action::SetTool(ToolId::Select)]);

        // Three-key sequence: E, A, O (Auto Place Outlets).
        out.clear();
        for k in [Key::E, Key::A, Key::O] {
            st.press(plain(k), t, &mut out);
        }
        assert_eq!(
            out,
            vec![Action::SetTool(ToolId::ElectricalVariant(
                crate::tools::electrical::ElecVariant::AutoOutlets
            ))]
        );
    }

    #[test]
    fn conflicts_are_detected_and_can_be_stolen() {
        let mut map = HotkeyMap::defaults();
        // The same sequence.
        let err = map
            .assign("Zoom Out", vec![plain(Key::Minus)], false)
            .unwrap_err();
        assert_eq!(err, vec!["Zoom In".to_string()]);
        assert!(map.sequences("Zoom Out").is_empty());
        // A prefix of an existing sequence (D vs D, H).
        let err = map
            .assign("Zoom Out", vec![plain(Key::D)], false)
            .unwrap_err();
        assert!(err.contains(&"Hinged Door".to_string()));
        // No clash: fine.
        map.assign("Zoom Out", vec![plain(Key::Equals)], false)
            .unwrap();
        assert_eq!(map.lookup(&[plain(Key::Equals)]), Some(Action::ZoomOut));
        // Stealing moves the chord.
        map.assign("Zoom Out", vec![plain(Key::Minus)], true)
            .unwrap();
        assert_eq!(map.lookup(&[plain(Key::Minus)]), Some(Action::ZoomOut));
        assert!(map.sequences("Zoom In").is_empty());
        // Four chords at most.
        let five = vec![plain(Key::A); 5];
        assert!(map.assign("Zoom In", five, false).is_err());
        // Reset restores the defaults.
        map.reset();
        assert_eq!(map.lookup(&[plain(Key::Minus)]), Some(Action::ZoomIn));
    }

    #[test]
    fn overrides_round_trip_through_the_file() {
        let path = temp_file("roundtrip");
        let mut map = HotkeyMap::defaults();
        assert!(map.overrides_json().contains("\"overrides\": {}"));
        map.assign("Zoom Out", vec![plain(Key::Equals)], false)
            .unwrap();
        map.remove("Hinged Door", &[plain(Key::D), plain(Key::H)]);
        let seq = vec![
            Chord {
                ctrl: true,
                alt: true,
                ..plain(Key::Num9)
            },
            plain(Key::A),
        ];
        map.assign("Window", seq.clone(), false).unwrap();
        map.save_to(&path).unwrap();

        let mut loaded = HotkeyMap::defaults();
        assert_eq!(
            loaded.lookup(&[plain(Key::D), plain(Key::H)]),
            Some(Action::SetTool(ToolId::Door))
        );
        let n = loaded.load_overrides(&path);
        assert_eq!(n, 3);
        assert_eq!(loaded.lookup(&[plain(Key::Equals)]), Some(Action::ZoomOut));
        assert_eq!(loaded.lookup(&[plain(Key::D), plain(Key::H)]), None);
        let seq: Vec<Chord> = seq.into_iter().map(Chord::normalized).collect();
        assert_eq!(loaded.lookup(&seq), Some(Action::SetTool(ToolId::Window)));
        assert_eq!(loaded.sequences("Window"), map.sequences("Window"));
        // The default chord of Window is still there beside the new one.
        assert!(loaded.sequences("Window").len() >= 2);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn bad_override_files_are_ignored() {
        let mut map = HotkeyMap::defaults();
        assert_eq!(map.apply_overrides_json("not json"), 0);
        assert_eq!(map.apply_overrides_json("{}"), 0);
        let n = map.apply_overrides_json(
            r#"{"version":1,"overrides":{"No Such Command":["Q"],"Zoom Out":["Bogus+++"]}}"#,
        );
        // The unknown command is skipped; the bad chord leaves Zoom Out unbound.
        assert_eq!(n, 1);
        assert!(map.sequences("Zoom Out").is_empty());
    }

    #[test]
    fn key_names_round_trip() {
        for k in [
            Key::A,
            Key::Num6,
            Key::F8,
            Key::Space,
            Key::Escape,
            Key::Delete,
            Key::Minus,
            Key::Backtick,
            Key::Backslash,
            Key::OpenBracket,
            Key::CloseBracket,
            Key::PageUp,
            Key::ArrowUp,
            Key::Tab,
        ] {
            let c = plain(k);
            assert_eq!(Chord::from_config(&c.to_config()), Some(c), "{k:?}");
        }
        let tilde = KeyChord::parse_file("~").unwrap();
        let c = Chord::from_config(&tilde).unwrap();
        assert!(c.shift && c.key == Key::Backtick);
    }

    #[test]
    fn commands_cover_the_tool_families() {
        let map = HotkeyMap::defaults();
        for name in [
            "Select Objects",
            "Straight Exterior Wall",
            "Straight Interior Wall",
            "Hinged Door",
            "Window",
            "Zoom In",
            "Zoom Out",
            "Fill Window",
            "Pan Window",
            "Undo",
            "Redo",
            "New Plan",
            "Open Plan",
            "Save",
            "Up One Floor",
            "Down One Floor",
            "Color",
            "Reference Grid",
            "Library Browser",
            "Customize Hotkeys",
            "Layer Display Options",
        ] {
            let c = map.command(name);
            assert!(c.is_some(), "missing command {name}");
        }
        assert!(map.command("Zoom In").unwrap().is_live());
        assert!(map.command("New Project").is_some(), "alias");
    }
}
