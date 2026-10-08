//! The toolbar configuration layer: which buttons and rows show, in which
//! order, per view type, saved in `~/.plan-studio/toolbars.json`.
//!
//! The data tables in the parent module ([`super::row1_slots`] and friends)
//! stay the single source of the buttons themselves (icon, action, flyout
//! entries, hotkey). This layer only decides which of them are drawn:
//!
//! * a **catalog** lists every available button (the three default bars plus
//!   flyouts that no default bar carries: terrain and landscape tools, fencing,
//!   images, the layout page tools), each filed in a group the way Chief files
//!   its toolbar buttons;
//! * a [`ToolbarConfig`] holds, for each [`ViewKind`] (floor plan, 3D view,
//!   vector elevation, layout), the three standard bars (`row1`, `row2`,
//!   `view`) and any rows the user added, each an ordered list of button keys
//!   with [`SEPARATOR`] between groups;
//! * [`draw_bar`] and [`draw_custom_rows`] are called by the renderer in the
//!   parent module and replace its plain left-to-right loop.
//!
//! A button's **key** is its name (`"Save"`), a flyout's group (`"Straight
//! Wall"`), or `"View Selector"` / `"Floor Number"`.
//!
//! The defaults reproduce the bars exactly as they were before this layer
//! existed (the floor-plan set); the other view types start from Daniel's Chief
//! per-view toolbar sets, reduced to what each view can use.

use super::{
    cad_blocks, distributed_objects, driveway, elevation_data, fencing, garden_bed, grass_region,
    image, plant, road, row1_slots, row2_slots, show_slot, sidewalk, sprinkler, stepping_stone,
    terrain_feature, terrain_modifier, terrain_wall_curb, view_slots, water_feature, Action,
    BarState, Slot,
};
use crate::shell::layout_window::LayoutCommand as L;
use eframe::egui;
use plan_config::{ChiefView, ToolbarSet};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The item key that stands for a separator line.
pub const SEPARATOR: &str = "|";
/// The settings file under `~/.plan-studio/`.
pub const FILE_NAME: &str = "toolbars.json";
/// Bumped when the file's shape changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

/// Row 1: file, edit, views, floors, 3D and materials.
pub const ROW1: &str = "row1";
/// Row 2: build tools, annotation and CAD.
pub const ROW2: &str = "row2";
/// The vertical bar on the right edge.
pub const VIEW_BAR: &str = "view";

/// First salt for the widget ids of buttons that do not come from the bar's
/// own slot list (see [`draw_items`]).
const POOL_ID_BASE: usize = 10_000;

// ----- view kinds -----

/// The kind of view a toolbar set belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    Plan,
    View3d,
    Elevation,
    Layout,
}

impl ViewKind {
    pub const ALL: [ViewKind; 4] = [
        ViewKind::Plan,
        ViewKind::View3d,
        ViewKind::Elevation,
        ViewKind::Layout,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ViewKind::Plan => "Floor Plan",
            ViewKind::View3d => "3D View",
            ViewKind::Elevation => "Vector Elevation",
            ViewKind::Layout => "Layout",
        }
    }

    /// The Chief view type whose toolbars an import reads for this kind.
    pub fn chief(self) -> ChiefView {
        match self {
            ViewKind::Plan => ChiefView::Plan,
            ViewKind::View3d => ChiefView::Camera3d,
            ViewKind::Elevation => ChiefView::Elevation,
            ViewKind::Layout => ChiefView::Layout,
        }
    }
}

// ----- the catalog -----

/// What a catalog entry draws as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Button,
    Toggle,
    Flyout,
    /// The saved-view drop-down and the floor number.
    Special,
}

impl EntryKind {
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::Button => "button",
            EntryKind::Toggle => "toggle",
            EntryKind::Flyout => "flyout",
            EntryKind::Special => "control",
        }
    }
}

/// One available button.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub key: &'static str,
    /// The Chief-style group it is filed under in the dialog.
    pub group: String,
    /// The default bar that carries it, `None` for buttons no default bar has.
    pub home: Option<&'static str>,
    pub kind: EntryKind,
    pub icon: &'static str,
    /// Flyout members (or the tooltip text of a plain button), for the dialog.
    pub detail: String,
    /// The names of a flyout's entries (empty for other buttons).
    pub members: Vec<&'static str>,
}

/// The key a slot is addressed by; `None` for separators.
pub fn slot_key(s: &Slot) -> Option<&'static str> {
    match s {
        Slot::Button(i) | Slot::Toggle(i) => Some(i.name),
        Slot::Flyout(f) => Some(f.group),
        Slot::ViewSelector => Some("View Selector"),
        Slot::FloorLabel => Some("Floor Number"),
        Slot::Separator => None,
    }
}

/// Group names, by separator-delimited run, for each default bar.
const ROW1_GROUPS: [&str; 11] = [
    "File",
    "Print and Layout",
    "Edit",
    "Program",
    "Saved Views",
    "Settings",
    "Floors",
    "3D Views and Cameras",
    "Sun Angle",
    "Materials",
    "Toolbar Configurations",
];
const ROW2_GROUPS: [&str; 11] = [
    "Select",
    "Walls and Railings",
    "Doors and Windows",
    "Cabinets and Electrical",
    "Stairs and Floors",
    "Roof, Framing and Structure",
    "Edit",
    "Dimensions",
    "Text and Notes",
    "CAD Drawing",
    "Detail",
];
const VIEW_GROUPS: [&str; 4] = ["Browsers", "Zoom", "Navigation", "Display Toggles"];

fn run_group(bar: &str, run: usize) -> String {
    let table: &[&str] = match bar {
        ROW1 => &ROW1_GROUPS,
        ROW2 => &ROW2_GROUPS,
        _ => &VIEW_GROUPS,
    };
    table.get(run).copied().unwrap_or("Other").to_string()
}

fn describe(slot: &Slot) -> (EntryKind, &'static str, String, Vec<&'static str>) {
    match slot {
        Slot::Button(i) => (EntryKind::Button, i.icon, i.name.to_string(), Vec::new()),
        Slot::Toggle(i) => (EntryKind::Toggle, i.icon, i.name.to_string(), Vec::new()),
        Slot::Flyout(f) => {
            let names: Vec<&'static str> = f.entries.iter().map(|e| e.name).collect();
            let icon = f.entries.first().map_or("select", |e| e.icon);
            (EntryKind::Flyout, icon, names.join(", "), names)
        }
        Slot::ViewSelector => (
            EntryKind::Special,
            "view_plan",
            "The saved plan view drop-down".into(),
            Vec::new(),
        ),
        Slot::FloorLabel => (
            EntryKind::Special,
            "floor_up",
            "The number of the floor being edited".into(),
            Vec::new(),
        ),
        Slot::Separator => (EntryKind::Special, "select", String::new(), Vec::new()),
    }
}

fn layout_item(icon: &'static str, name: &'static str, cmd: L) -> Slot {
    Slot::Button(super::item(icon, name, Action::Layout(cmd)))
}

/// Buttons for the layout view: page tools and box tools.
fn layout_slots() -> Vec<Slot> {
    vec![
        layout_item("file_new", "Insert Page After", L::InsertPageAfter),
        layout_item("file_new", "Insert Page Before", L::InsertPageBefore),
        layout_item("drawing_sheet", "Duplicate Page", L::DuplicatePage),
        layout_item("floor_delete", "Delete Page", L::DeletePage),
        layout_item("floor_up", "Exchange With Next Page", L::ExchangeWithNext),
        layout_item(
            "floor_down",
            "Exchange With Previous Page",
            L::ExchangeWithPrevious,
        ),
        layout_item("floor_down", "Previous Page", L::PreviousPage),
        layout_item("floor_up", "Next Page", L::NextPage),
        layout_item("plan_database", "Page Table", L::PageTable),
        layout_item("default_settings", "Page Setup", L::PageSetup),
        layout_item("view_save", "Update Layout Views", L::UpdateViews),
        layout_item("fill_window", "Fit Page in Window", L::FitPage),
        layout_item("note", "Add Text Box", L::AddTextBox),
        layout_item("view_plan", "Add Picture", L::AddImageBox),
        layout_item("material_editor", "Add Materials List", L::AddMaterialsBox),
        layout_item("plan_database", "Add Sheet Index", L::AddSheetIndex),
        layout_item("layer_display", "Layout Layer Display", L::LayerDisplay),
        layout_item("file_print", "Print Layout", L::Print),
        layout_item("file_print", "Print Image", L::PrintImage),
        layout_item("file_print", "Export Layout PDF", L::ExportPdf),
    ]
}

struct Source {
    home: Option<&'static str>,
    group: String,
    slot: Slot,
}

/// Every available button with its slot, default bars first.
fn sources() -> Vec<Source> {
    let mut out = Vec::new();
    for (bar, slots) in [
        (ROW1, row1_slots()),
        (ROW2, row2_slots()),
        (VIEW_BAR, view_slots()),
    ] {
        let mut run = 0;
        for slot in slots {
            if matches!(slot, Slot::Separator) {
                run += 1;
                continue;
            }
            out.push(Source {
                home: Some(bar),
                group: run_group(bar, run),
                slot,
            });
        }
    }
    let extras: Vec<(&str, Slot)> = vec![
        ("Walls and Railings", Slot::Flyout(fencing())),
        ("Images and Objects", Slot::Flyout(image())),
        ("Images and Objects", Slot::Flyout(distributed_objects())),
        ("CAD Drawing", Slot::Flyout(cad_blocks())),
        ("Terrain and Landscape", Slot::Flyout(terrain_wall_curb())),
        ("Terrain and Landscape", Slot::Flyout(elevation_data())),
        ("Terrain and Landscape", Slot::Flyout(terrain_modifier())),
        ("Terrain and Landscape", Slot::Flyout(terrain_feature())),
        ("Terrain and Landscape", Slot::Flyout(garden_bed())),
        ("Terrain and Landscape", Slot::Flyout(grass_region())),
        ("Terrain and Landscape", Slot::Flyout(water_feature())),
        ("Terrain and Landscape", Slot::Flyout(stepping_stone())),
        ("Terrain and Landscape", Slot::Flyout(road())),
        ("Terrain and Landscape", Slot::Flyout(driveway())),
        ("Terrain and Landscape", Slot::Flyout(sidewalk())),
        ("Terrain and Landscape", Slot::Flyout(plant())),
        ("Terrain and Landscape", Slot::Flyout(sprinkler())),
    ];
    for (group, slot) in extras {
        out.push(Source {
            home: None,
            group: group.to_string(),
            slot,
        });
    }
    for slot in layout_slots() {
        out.push(Source {
            home: None,
            group: "Layout Pages".to_string(),
            slot,
        });
    }
    out
}

/// All available buttons, grouped in the order the dialog lists them.
pub fn catalog() -> &'static [CatalogEntry] {
    static CATALOG: OnceLock<Vec<CatalogEntry>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        sources()
            .into_iter()
            .filter_map(|s| {
                let key = slot_key(&s.slot)?;
                let (kind, icon, detail, members) = describe(&s.slot);
                Some(CatalogEntry {
                    key,
                    group: s.group,
                    home: s.home,
                    kind,
                    icon,
                    detail,
                    members,
                })
            })
            .collect()
    })
}

/// The catalog entry for `key`.
pub fn entry(key: &str) -> Option<&'static CatalogEntry> {
    catalog().iter().find(|e| e.key == key)
}

/// The group names in dialog order, without repeats.
pub fn groups() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in catalog() {
        if !out.contains(&e.group) {
            out.push(e.group.clone());
        }
    }
    out
}

/// A fresh slot for `key`.
pub fn build_slot(key: &str) -> Option<Slot> {
    sources()
        .into_iter()
        .map(|s| s.slot)
        .find(|s| slot_key(s) == Some(key))
}

/// The keys the default bar `bar` carries, in default order.
pub fn native_keys(bar: &str) -> Vec<&'static str> {
    catalog()
        .iter()
        .filter(|e| e.home == Some(bar))
        .map(|e| e.key)
        .collect()
}

fn is_native(bar: &str, key: &str) -> bool {
    catalog()
        .iter()
        .any(|e| e.key == key && e.home == Some(bar))
}

/// Which default bar a slot list is, judged by its first button. The slot
/// lists are the full defaults and never filtered, so this is stable.
pub fn bar_of(slots: &[Slot]) -> Option<&'static str> {
    match slots.iter().find_map(slot_key)? {
        "New Plan" => Some(ROW1),
        "Select Objects" => Some(ROW2),
        "Library Browser" => Some(VIEW_BAR),
        _ => None,
    }
}

// ----- the configuration -----

fn yes() -> bool {
    true
}

/// One toolbar row (or the vertical bar): an ordered list of keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BarLayout {
    /// [`ROW1`], [`ROW2`], [`VIEW_BAR`], or `custom-N` for a row the user added.
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Button keys in order, [`SEPARATOR`] between groups.
    pub items: Vec<String>,
    /// Buttons of a standard bar that the user turned off (kept so a button
    /// added by a later version still shows up by itself).
    #[serde(default)]
    pub hidden: Vec<String>,
}

pub fn is_builtin(id: &str) -> bool {
    matches!(id, ROW1 | ROW2 | VIEW_BAR)
}

impl BarLayout {
    fn new(id: &str, name: &str, items: Vec<String>) -> BarLayout {
        let mut b = BarLayout {
            id: id.to_string(),
            name: name.to_string(),
            visible: true,
            items,
            hidden: Vec::new(),
        };
        b.tidy();
        b.recompute_hidden();
        b
    }

    pub fn is_builtin(&self) -> bool {
        is_builtin(&self.id)
    }

    /// Replaces the list; a standard bar's `hidden` follows.
    #[cfg(test)]
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.tidy();
        self.recompute_hidden();
    }

    pub fn contains(&self, key: &str) -> bool {
        self.items.iter().any(|k| k == key)
    }

    /// The button keys, without separators.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.items.iter().filter(|k| *k != SEPARATOR)
    }

    /// Removes leading, trailing and doubled separators.
    pub fn tidy(&mut self) {
        let mut out: Vec<String> = Vec::with_capacity(self.items.len());
        for k in self.items.drain(..) {
            if k == SEPARATOR && out.last().is_none_or(|p| p == SEPARATOR) {
                continue;
            }
            out.push(k);
        }
        while out.last().is_some_and(|k| k == SEPARATOR) {
            out.pop();
        }
        self.items = out;
    }

    /// `hidden` = the standard bar's own buttons that are not shown.
    fn recompute_hidden(&mut self) {
        self.hidden = if self.is_builtin() {
            native_keys(&self.id)
                .into_iter()
                .filter(|k| !self.contains(k))
                .map(String::from)
                .collect()
        } else {
            Vec::new()
        };
    }

    /// Shows `key` at position `at` (default: the end). False when it is
    /// already there or not an available button.
    pub fn add(&mut self, key: &str, at: Option<usize>) -> bool {
        if key == SEPARATOR || self.contains(key) || entry(key).is_none() {
            return false;
        }
        let at = at.unwrap_or(self.items.len()).min(self.items.len());
        self.items.insert(at, key.to_string());
        self.hidden.retain(|k| k != key);
        self.tidy();
        true
    }

    /// Adds a separator before position `at`.
    pub fn add_separator(&mut self, at: usize) {
        let at = at.min(self.items.len());
        self.items.insert(at, SEPARATOR.to_string());
        self.tidy();
    }

    /// Takes the entry at `i` off the bar.
    pub fn remove_at(&mut self, i: usize) {
        if i >= self.items.len() {
            return;
        }
        let key = self.items.remove(i);
        if key != SEPARATOR && is_native(&self.id, &key) && !self.hidden.contains(&key) {
            self.hidden.push(key);
        }
        self.tidy();
    }

    /// Takes `key` off the bar.
    pub fn remove_key(&mut self, key: &str) -> bool {
        match self.items.iter().position(|k| k == key) {
            Some(i) => {
                self.remove_at(i);
                true
            }
            None => false,
        }
    }

    /// Moves the entry at `from` to `to` (an index in the list as it is after
    /// the move). Returns false when either is out of range.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() {
            return false;
        }
        let k = self.items.remove(from);
        self.items.insert(to, k);
        self.tidy();
        true
    }
}

/// The bars of one view type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewSet {
    pub bars: Vec<BarLayout>,
}

impl ViewSet {
    pub fn bar(&self, id: &str) -> Option<&BarLayout> {
        self.bars.iter().find(|b| b.id == id)
    }

    pub fn bar_mut(&mut self, id: &str) -> Option<&mut BarLayout> {
        self.bars.iter_mut().find(|b| b.id == id)
    }

    /// Adds an empty row named `name` and returns its id.
    pub fn add_row(&mut self, name: &str) -> String {
        let mut n = 1;
        let id = loop {
            let id = format!("custom-{n}");
            if self.bar(&id).is_none() {
                break id;
            }
            n += 1;
        };
        let name = if name.trim().is_empty() {
            format!("Row {}", self.bars.len() + 1)
        } else {
            name.trim().to_string()
        };
        self.bars.push(BarLayout::new(&id, &name, Vec::new()));
        id
    }

    /// Renames a row (the standard ones keep their id). False for an empty
    /// name or an unknown row.
    pub fn rename_row(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        match self.bar_mut(id) {
            Some(b) if !name.is_empty() => {
                b.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Moves a row the user added one place up or down among the user's
    /// rows (the standard bars stay first, in their order).
    pub fn move_row(&mut self, id: &str, down: bool) -> bool {
        if is_builtin(id) {
            return false;
        }
        let custom: Vec<usize> = self
            .bars
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.is_builtin())
            .map(|(i, _)| i)
            .collect();
        let Some(at) = custom.iter().position(|&i| self.bars[i].id == id) else {
            return false;
        };
        let to = if down {
            if at + 1 >= custom.len() {
                return false;
            }
            at + 1
        } else {
            if at == 0 {
                return false;
            }
            at - 1
        };
        self.bars.swap(custom[at], custom[to]);
        true
    }

    /// Adds a copy of a row (its buttons and separators) after the user's
    /// last row and returns the new row's id.
    pub fn duplicate_row(&mut self, id: &str) -> Option<String> {
        let src = self.bar(id)?.clone();
        let new_id = self.add_row(&format!("{} copy", src.name));
        let bar = self.bar_mut(&new_id)?;
        bar.items = src.items;
        bar.visible = src.visible;
        bar.tidy();
        Some(new_id)
    }

    /// Deletes a row the user added. The standard bars cannot be deleted
    /// (hide them instead).
    pub fn remove_row(&mut self, id: &str) -> bool {
        if is_builtin(id) {
            return false;
        }
        let before = self.bars.len();
        self.bars.retain(|b| b.id != id);
        self.bars.len() != before
    }
}

/// Everything saved in `toolbars.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolbarConfig {
    pub version: u32,
    /// Lock Toolbars: the Customize Toolbars dialog refuses edits.
    #[serde(default)]
    pub locked: bool,
    pub views: BTreeMap<ViewKind, ViewSet>,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        ToolbarConfig::daniel_default()
    }
}

fn keys_of(slots: &[Slot]) -> Vec<String> {
    slots
        .iter()
        .map(|s| slot_key(s).unwrap_or(SEPARATOR).to_string())
        .collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// `items` without the listed keys.
fn without(items: Vec<String>, drop: &[&str]) -> Vec<String> {
    items
        .into_iter()
        .filter(|k| !drop.contains(&k.as_str()))
        .collect()
}

fn set_of(row1: Vec<String>, row2: Vec<String>, view: Vec<String>) -> ViewSet {
    ViewSet {
        bars: vec![
            BarLayout::new(ROW1, "File and View", row1),
            BarLayout::new(ROW2, "Build and Annotate", row2),
            BarLayout::new(VIEW_BAR, "Navigation", view),
        ],
    }
}

const SAVED_VIEW_KEYS: [&str; 4] = [
    "Edit Active View",
    "Save Active View",
    "Save Active View As",
    "View Selector",
];
const FLOOR_KEYS: [&str; 4] = [
    "Floor Defaults",
    "Down One Floor",
    "Floor Number",
    "Up One Floor",
];
const CAD_KEYS: [&str; 10] = [
    "Points",
    "Lines",
    "Arcs",
    "Circles",
    "Boxes",
    "Spline",
    "Revision Cloud",
    "Schedule",
    "Auto Detail",
    "Current CAD Layer",
];

impl ToolbarConfig {
    /// The shipped sets: the floor plan as the bars have always been (Daniel's
    /// Chief Default Configuration), and the other view types reduced from it.
    pub fn daniel_default() -> ToolbarConfig {
        let mut views = BTreeMap::new();
        for v in ViewKind::ALL {
            views.insert(v, ToolbarConfig::default_view(v));
        }
        ToolbarConfig {
            version: FORMAT_VERSION,
            locked: false,
            views,
        }
    }

    /// The shipped set of one view type.
    pub fn default_view(view: ViewKind) -> ViewSet {
        let row1 = keys_of(&row1_slots());
        let row2 = keys_of(&row2_slots());
        let bar = keys_of(&view_slots());
        match view {
            ViewKind::Plan => set_of(row1, row2, bar),
            // A 3D view keeps the build tools (they switch back to the plan)
            // but not the plan-only drawing tools or the saved plan views.
            ViewKind::View3d => set_of(
                without(row1, &SAVED_VIEW_KEYS),
                without(row2, &CAD_KEYS),
                bar,
            ),
            // A vector elevation is annotated and drawn on, not built in.
            ViewKind::Elevation => set_of(
                without(without(row1, &SAVED_VIEW_KEYS), &FLOOR_KEYS),
                strs(&[
                    "Select Objects",
                    "|",
                    "Paste Hold Position",
                    "|",
                    "Dimensions",
                    "Automatic Dimensions",
                    "|",
                    "Text",
                    "Revision Cloud",
                    "|",
                    "Points",
                    "Lines",
                    "Arcs",
                    "Circles",
                    "Boxes",
                    "Spline",
                    "|",
                    "Auto Detail",
                    "Current CAD Layer",
                ]),
                bar,
            ),
            ViewKind::Layout => {
                let mut s = set_of(
                    strs(&[
                        "New Plan",
                        "Open Plan",
                        "Save",
                        "|",
                        "Print",
                        "Send to Layout",
                        "|",
                        "Undo",
                        "Redo",
                        "|",
                        "Preferences",
                        "Launch Help",
                        "|",
                        "Display Options",
                        "Default Settings",
                    ]),
                    strs(&[
                        "Insert Page After",
                        "Insert Page Before",
                        "Duplicate Page",
                        "Delete Page",
                        "|",
                        "Exchange With Previous Page",
                        "Exchange With Next Page",
                        "|",
                        "Previous Page",
                        "Next Page",
                        "Page Table",
                        "|",
                        "Page Setup",
                        "Update Layout Views",
                        "Fit Page in Window",
                        "|",
                        "Add Text Box",
                        "Add Picture",
                        "Add Materials List",
                        "Add Sheet Index",
                        "|",
                        "Export Layout PDF",
                    ]),
                    strs(&[
                        "Library Browser",
                        "Project Browser",
                        "Active Layer Display Options",
                    ]),
                );
                if let Some(b) = s.bar_mut(ROW2) {
                    b.name = "Layout Pages".into();
                }
                s
            }
        }
    }

    pub fn view(&self, v: ViewKind) -> Option<&ViewSet> {
        self.views.get(&v)
    }

    pub fn view_mut(&mut self, v: ViewKind) -> &mut ViewSet {
        self.views
            .entry(v)
            .or_insert_with(|| ToolbarConfig::default_view(v))
    }

    /// Back to the shipped set of one view type.
    pub fn reset_view(&mut self, v: ViewKind) {
        self.views.insert(v, ToolbarConfig::default_view(v));
    }

    /// Back to the shipped sets (the lock is kept).
    pub fn reset_all(&mut self) {
        let locked = self.locked;
        *self = ToolbarConfig::daniel_default();
        self.locked = locked;
    }

    /// Repairs a configuration read from disk: missing views and standard
    /// bars come back from the defaults, keys that no longer exist and
    /// repeats are dropped, and buttons the file has never heard of (added by
    /// a newer version) are shown where they sit in the default order.
    pub fn normalize(&mut self) {
        self.version = FORMAT_VERSION;
        for v in ViewKind::ALL {
            let default = ToolbarConfig::default_view(v);
            let set = self.views.entry(v).or_insert_with(|| default.clone());
            // Standard bars first, in order, then the user's rows.
            for id in [ROW1, ROW2, VIEW_BAR] {
                if set.bar(id).is_none() {
                    if let Some(b) = default.bar(id) {
                        set.bars.push(b.clone());
                    }
                }
            }
            let mut seen_ids = HashSet::new();
            set.bars.retain(|b| seen_ids.insert(b.id.clone()));
            set.bars.sort_by_key(|b| match b.id.as_str() {
                ROW1 => 0,
                ROW2 => 1,
                VIEW_BAR => 2,
                _ => 3,
            });
            for b in &mut set.bars {
                let mut seen = HashSet::new();
                b.items
                    .retain(|k| k == SEPARATOR || (entry(k).is_some() && seen.insert(k.clone())));
                b.hidden.retain(|k| entry(k).is_some());
                if b.is_builtin() {
                    add_unseen_native(b);
                }
                b.tidy();
            }
        }
    }

    /// The shown keys of one bar for the renderer: `None` when the view or
    /// bar is unknown, otherwise whether it is visible and its items.
    fn items_for(&self, view: ViewKind, bar: &str) -> Option<(bool, Vec<String>)> {
        let b = self.views.get(&view)?.bar(bar)?;
        Some((b.visible, b.items.clone()))
    }

    // ----- persistence -----

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<ToolbarConfig, String> {
        let mut cfg: ToolbarConfig =
            serde_json::from_str(text).map_err(|e| format!("not a toolbar file: {e}"))?;
        cfg.normalize();
        Ok(cfg)
    }

    pub fn load_from(path: &Path) -> Result<ToolbarConfig, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        ToolbarConfig::from_json(&text)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, self.to_json()).map_err(|e| e.to_string())
    }
}

/// Shows the default-bar buttons that are neither listed nor hidden, after
/// their nearest listed predecessor in default order.
fn add_unseen_native(b: &mut BarLayout) {
    let native = native_keys(&b.id);
    for (j, key) in native.iter().enumerate() {
        if b.contains(key) || b.hidden.iter().any(|h| h == key) {
            continue;
        }
        let after = native[..j]
            .iter()
            .rev()
            .find_map(|p| b.items.iter().position(|k| k == p));
        b.items
            .insert(after.map_or(0, |i| i + 1), (*key).to_string());
    }
}

// ----- the live configuration -----

thread_local! {
    static CURRENT: RefCell<Option<ToolbarConfig>> = const { RefCell::new(None) };
    static VIEW: Cell<ViewKind> = const { Cell::new(ViewKind::Plan) };
    /// Slots of buttons drawn on a bar that does not carry them by default.
    static POOL: RefCell<HashMap<String, Slot>> = RefCell::new(HashMap::new());
    /// The keys drawn in the last frame (tests only).
    #[cfg(test)]
    static DRAWN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// `~/.plan-studio/toolbars.json`.
pub fn user_path() -> Option<PathBuf> {
    crate::paths::user_file(FILE_NAME)
}

fn initial() -> ToolbarConfig {
    // Unit tests never read the real settings folder.
    if cfg!(test) {
        return ToolbarConfig::default();
    }
    user_path()
        .filter(|p| p.exists())
        .and_then(|p| ToolbarConfig::load_from(&p).ok())
        .unwrap_or_default()
}

fn with_current<R>(f: impl FnOnce(&mut ToolbarConfig) -> R) -> R {
    CURRENT.with(|c| f(c.borrow_mut().get_or_insert_with(initial)))
}

/// A copy of the live configuration.
pub fn current() -> ToolbarConfig {
    with_current(|c| c.clone())
}

/// Makes `cfg` the live configuration and saves it to `toolbars.json`.
pub fn set_current(mut cfg: ToolbarConfig) -> Result<(), String> {
    cfg.normalize();
    let path = user_path();
    let saved = if cfg!(test) {
        Ok(())
    } else {
        match &path {
            Some(p) => cfg.save_to(p),
            None => Err(crate::paths::NO_HOME.to_string()),
        }
    };
    with_current(|c| *c = cfg);
    saved
}

/// Reset Toolbars: every view type back to Daniel's Chief set (the lock is
/// kept), live and saved. Returns the save error, if any.
pub fn reset_all_live() -> Result<(), String> {
    let mut cfg = current();
    cfg.reset_all();
    set_current(cfg)
}

/// Replaces the live configuration without touching the disk (tests).
#[cfg(test)]
pub fn set_current_unsaved(mut cfg: ToolbarConfig) {
    cfg.normalize();
    with_current(|c| *c = cfg);
}

/// Tells the layer which kind of 3D or plan view is on screen. The layout is
/// recognised by itself (`shell::layout_window::is_active`).
pub fn set_active_view(v: ViewKind) {
    VIEW.with(|c| c.set(v));
}

/// The view type whose set is drawn now.
pub fn active_view() -> ViewKind {
    if crate::shell::layout_window::is_active() {
        ViewKind::Layout
    } else {
        VIEW.with(Cell::get)
    }
}

// ----- drawing -----

/// Draws the standard bar `slots` according to the live configuration, in the
/// current layout direction. False when `slots` is not one of the standard
/// bars (the caller then draws it as it is).
pub(super) fn draw_bar(
    ui: &mut egui::Ui,
    slots: &mut [Slot],
    state: &BarState,
    out: &mut Vec<Action>,
) -> bool {
    let Some(bar) = bar_of(slots) else {
        return false;
    };
    let view = active_view();
    let Some((visible, items)) = with_current(|c| c.items_for(view, bar)) else {
        return false;
    };
    if visible {
        draw_items(ui, Some(slots), &items, 0, state, out);
    }
    true
}

/// Draws the rows the user added under the build bar.
pub(super) fn draw_custom_rows(
    ui: &mut egui::Ui,
    slots: &[Slot],
    state: &BarState,
    out: &mut Vec<Action>,
) {
    if bar_of(slots) != Some(ROW2) {
        return;
    }
    let view = active_view();
    let rows: Vec<(usize, Vec<String>)> = with_current(|c| {
        c.view(view)
            .map(|s| {
                s.bars
                    .iter()
                    .filter(|b| !b.is_builtin() && b.visible && !b.items.is_empty())
                    .enumerate()
                    .map(|(n, b)| (n + 1, b.items.clone()))
                    .collect()
            })
            .unwrap_or_default()
    });
    for (n, items) in rows {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::new(super::GAP_PX, 0.0);
            draw_items(ui, None, &items, n, state, out);
        });
    }
}

/// Draws `items`: buttons found in `own` keep their state there, the rest
/// live in a per-key pool. `bar_no` salts the pool widgets' ids.
fn draw_items(
    ui: &mut egui::Ui,
    mut own: Option<&mut [Slot]>,
    items: &[String],
    bar_no: usize,
    state: &BarState,
    out: &mut Vec<Action>,
) {
    let mut drawn_any = false;
    let mut pending_sep = false;
    for (pos, key) in items.iter().enumerate() {
        if key == SEPARATOR {
            pending_sep = drawn_any;
            continue;
        }
        let in_own = own
            .as_deref()
            .and_then(|s| s.iter().position(|sl| slot_key(sl) == Some(key.as_str())));
        let shown = match (in_own, own.as_deref_mut()) {
            (Some(i), Some(slots)) => {
                if pending_sep {
                    super::separator(ui);
                }
                show_slot(ui, i, &mut slots[i], state, out);
                true
            }
            _ => POOL.with(|p| {
                let mut p = p.borrow_mut();
                if !p.contains_key(key) {
                    if let Some(s) = build_slot(key) {
                        p.insert(key.clone(), s);
                    }
                }
                match p.get_mut(key) {
                    Some(slot) => {
                        if pending_sep {
                            super::separator(ui);
                        }
                        show_slot(ui, POOL_ID_BASE + bar_no * 1000 + pos, slot, state, out);
                        true
                    }
                    None => false,
                }
            }),
        };
        if shown {
            #[cfg(test)]
            DRAWN.with(|d| d.borrow_mut().push(key.clone()));
            drawn_any = true;
            pending_sep = false;
        }
    }
}

// ----- Chief toolbar import -----

/// Chief button labels (as `plan_config::clean_label` leaves them) and the
/// key of the Plan Studio button that stands for each.
const CHIEF_LABELS: &[(&str, &str)] = &[
    ("new plan", "New Plan"),
    ("new project", "New Plan"),
    ("open plan", "Open Plan"),
    ("open plan/layout", "Open Plan"),
    ("save", "Save"),
    ("save entire project", "Save"),
    ("print", "Print"),
    ("send to layout", "Send to Layout"),
    ("print image", "Print Image"),
    ("undo", "Undo"),
    ("redo", "Redo"),
    ("preferences", "Preferences"),
    ("launch help", "Launch Help"),
    ("edit active view", "Edit Active View"),
    ("save active view", "Save Active View"),
    ("save active view as", "Save Active View As"),
    ("saved plan view control", "View Selector"),
    ("display options", "Display Options"),
    ("default settings", "Default Settings"),
    ("referenced plans/layouts", "Plan Database"),
    ("floor defaults", "Floor Defaults"),
    ("down one floor", "Down One Floor"),
    ("up one floor", "Up One Floor"),
    ("change floor/reference", "Floor Number"),
    ("orthographic view tools", "3D View"),
    ("camera view tools", "Full Camera"),
    ("move camera with mouse", "Mouse-Orbit Camera"),
    ("camera view options", "Cross Section Slider"),
    ("walkthrough tools", "Create Walkthrough Path"),
    ("rendering techniques", "Rendering Techniques"),
    ("camera view lighting tools", "Add Lights"),
    ("sun angle", "Sun Angle"),
    ("material painter", "Material Painter"),
    ("material eyedropper", "Material Eyedropper"),
    ("object eyedropper", "Object Eyedropper"),
    ("delete surface", "Delete Surface"),
    ("adjust material definition", "Adjust Material Definition"),
    ("interactive material editor", "Interactive Material Editor"),
    ("default configuration", "Default Configuration"),
    (
        "space planning configuration",
        "Space Planning Configuration",
    ),
    ("extended tool configuration", "Extended Tool Configuration"),
    ("select objects", "Select Objects"),
    ("straight wall tools", "Straight Wall"),
    ("railing and deck tools", "Railing and Deck"),
    ("curved wall tools", "Curved Wall"),
    ("door tools", "Door"),
    ("window tools", "Window"),
    ("cabinet tools", "Cabinet"),
    ("electrical tools", "Electrical"),
    ("stair tools", "Stairs"),
    ("floor tools", "Floor"),
    ("roof tools", "Roof"),
    ("trim tools", "Trim"),
    ("general framing tools", "General Framing"),
    ("floor/ceiling framing tools", "Floor/Ceiling Framing"),
    ("roof framing tools", "Roof Framing"),
    ("slab tools", "Slab"),
    ("3d solid tools", "3D Solid"),
    ("paste hold position", "Paste Hold Position"),
    ("dimension tools", "Dimensions"),
    ("automatic dimension tools", "Automatic Dimensions"),
    ("text tools", "Text"),
    ("revision cloud", "Revision Cloud"),
    ("point tools", "Points"),
    ("line tools", "Lines"),
    ("arc tools", "Arcs"),
    ("circle tools", "Circles"),
    ("box tools", "Boxes"),
    ("spline", "Spline"),
    ("auto detail", "Auto Detail"),
    ("current cad layer", "Current CAD Layer"),
    ("library browser", "Library Browser"),
    ("project browser", "Project Browser"),
    (
        "active layer display options",
        "Active Layer Display Options",
    ),
    ("zoom", "Zoom"),
    ("zoom in", "Zoom In"),
    ("zoom out", "Zoom Out"),
    ("undo zoom", "Undo Zoom"),
    (
        "fill window selected objects",
        "Fill Window Selected Objects",
    ),
    ("fill window building only", "Fill Window Building Only"),
    ("fill window", "Fill Window"),
    ("pan window", "Pan Window"),
    ("reference display", "Reference Display"),
    ("crosshairs", "Crosshairs"),
    ("color", "Color"),
    ("line weights", "Line Weights"),
    ("drawing sheet", "Drawing Sheet"),
    ("print preview", "Print Preview"),
    ("temporary dimensions", "Temporary Dimensions"),
    ("connect cad segments", "Connect CAD Segments"),
    ("arc centers and ends", "Arc Centers and Ends"),
    ("insert page after", "Insert Page After"),
    ("insert page before", "Insert Page Before"),
    ("duplicate page", "Duplicate Page"),
    ("delete page", "Delete Page"),
    ("exchange with next page", "Exchange With Next Page"),
    ("exchange with previous page", "Exchange With Previous Page"),
    ("page down", "Next Page"),
    ("page up", "Previous Page"),
    ("layout page table", "Page Table"),
    ("edit page information", "Page Setup"),
    ("update layout views", "Update Layout Views"),
    ("export materials list", "Add Materials List"),
    ("terrain tools", "Elevation Data"),
    ("terrain elevation tools", "Elevation Data"),
    ("terrain modifier tools", "Modifier"),
    ("terrain feature tools", "Feature"),
    ("garden bed tools", "Garden Bed"),
    ("grass tools", "Grass Region"),
    ("water feature tools", "Water Feature"),
    ("fencing tools", "Fencing"),
    ("terrain wall and curb tools", "Terrain Wall and Curb"),
    ("road tools", "Road"),
    ("driveway tools", "Driveway"),
    ("sidewalk tools", "Sidewalk"),
    ("stepping stone tools", "Stepping Stone"),
    ("plant tools", "Plant"),
    ("sprinkler tools", "Sprinkler"),
    ("schedule tools", "Schedule"),
];

/// The Plan Studio button that stands for a Chief button label.
pub fn map_chief_label(label: &str) -> Option<&'static str> {
    let l = label.trim().to_lowercase();
    CHIEF_LABELS
        .iter()
        .find(|(chief, _)| *chief == l)
        .map(|(_, key)| *key)
        .or_else(|| entry_ci(&l))
        .or_else(|| member_ci(&l))
}

/// The flyout that has an entry named like a Chief button (Chief's Extended
/// set lists single tools such as "Arc About Center" that Plan Studio keeps
/// in a flyout).
fn member_ci(lower: &str) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|e| e.members.iter().any(|m| m.to_lowercase() == lower))
        .map(|e| e.key)
}

fn entry_ci(lower: &str) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|e| e.key.to_lowercase() == lower)
        .map(|e| e.key)
}

/// What an import found.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportReport {
    pub view: ViewKind,
    /// Chief buttons in the file's toolbars for this view type.
    pub total: usize,
    /// Of those, buttons with a Plan Studio counterpart.
    pub mapped: usize,
    /// Chief labels with none.
    pub unmapped: Vec<String>,
    /// The resulting standard bars.
    pub set: ViewSet,
}

impl ImportReport {
    /// A line for the dialog.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} of {} Chief buttons have a Plan Studio counterpart{}",
            self.view.label(),
            self.mapped,
            self.total,
            if self.unmapped.is_empty() {
                String::new()
            } else {
                format!(" (no counterpart: {})", self.unmapped.join(", "))
            }
        )
    }
}

/// Maps the toolbars `set` shows in `view`'s Chief view type onto Plan
/// Studio's three standard bars: every mapped button goes on the bar that
/// carries it by default, in Chief's order, with a separator between Chief
/// toolbars. Buttons the file does not list are hidden.
pub fn import_chief(set: &ToolbarSet, view: ViewKind) -> ImportReport {
    let mut bars: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut total = 0;
    let mut mapped = 0;
    let mut unmapped = Vec::new();
    let mut placed: HashSet<&'static str> = HashSet::new();
    for tb in plan_config::buttons_for_view(set, view.chief().code()) {
        let mut touched: Vec<&'static str> = Vec::new();
        for (_, label) in tb.buttons {
            total += 1;
            match map_chief_label(&label) {
                Some(key) => {
                    mapped += 1;
                    if !placed.insert(key) {
                        continue;
                    }
                    let home = entry(key).and_then(|e| e.home).unwrap_or(ROW2);
                    bars.entry(home).or_default().push(key.to_string());
                    if !touched.contains(&home) {
                        touched.push(home);
                    }
                }
                None => unmapped.push(label),
            }
        }
        for home in touched {
            if let Some(items) = bars.get_mut(home) {
                items.push(SEPARATOR.to_string());
            }
        }
    }
    let mut take = |id: &str| bars.remove(id).unwrap_or_default();
    let (r1, r2, vb) = (take(ROW1), take(ROW2), take(VIEW_BAR));
    unmapped.dedup();
    ImportReport {
        view,
        total,
        mapped,
        unmapped,
        set: set_of(r1, r2, vb),
    }
}

impl ToolbarConfig {
    /// Replaces the standard bars of `report.view` with the imported ones;
    /// rows the user added are kept.
    pub fn apply_import(&mut self, report: &ImportReport) {
        let set = self.view_mut(report.view);
        for imported in &report.set.bars {
            if let Some(b) = set.bar_mut(&imported.id) {
                b.items = imported.items.clone();
                b.hidden = imported.hidden.clone();
                b.visible = !imported.items.is_empty();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_TOOLBAR: &str =
        include_str!("../../../../docs/chief-config-raw/Default Configuration.toolbar");

    fn set() -> ToolbarSet {
        plan_config::parse_toolbar_named("Default Configuration", DEFAULT_TOOLBAR).unwrap()
    }

    #[test]
    fn keys_are_unique_and_every_group_is_named() {
        let mut seen = HashSet::new();
        for e in catalog() {
            assert!(seen.insert(e.key), "duplicate key {}", e.key);
            assert_ne!(e.group, "Other", "{} has no group", e.key);
            assert_ne!(e.key, SEPARATOR);
        }
        assert!(catalog().len() > 120, "{}", catalog().len());
        // The default bars are all there.
        assert_eq!(
            native_keys(ROW1).len() + native_keys(ROW2).len() + native_keys(VIEW_BAR).len(),
            catalog().iter().filter(|e| e.home.is_some()).count()
        );
    }

    #[test]
    fn every_catalog_key_builds_a_slot() {
        for e in catalog() {
            let s = build_slot(e.key).unwrap_or_else(|| panic!("no slot for {}", e.key));
            assert_eq!(slot_key(&s), Some(e.key));
        }
        assert!(build_slot("No Such Button").is_none());
    }

    #[test]
    fn the_plan_defaults_reproduce_the_existing_bars() {
        let cfg = ToolbarConfig::daniel_default();
        let plan = cfg.view(ViewKind::Plan).unwrap();
        for (id, slots) in [
            (ROW1, row1_slots()),
            (ROW2, row2_slots()),
            (VIEW_BAR, view_slots()),
        ] {
            let b = plan.bar(id).unwrap();
            // Same buttons in the same order; separators only differ by
            // being tidied.
            let want: Vec<&str> = slots.iter().filter_map(slot_key).collect();
            let got: Vec<&str> = b.keys().map(String::as_str).collect();
            assert_eq!(got, want, "{id}");
            assert!(b.hidden.is_empty());
            assert_eq!(bar_of(&slots), Some(id));
        }
    }

    #[test]
    fn views_differ_where_the_view_needs_it() {
        let cfg = ToolbarConfig::daniel_default();
        let get = |v: ViewKind, bar: &str, key: &str| {
            cfg.view(v).unwrap().bar(bar).unwrap().contains(key)
        };
        assert!(get(ViewKind::Plan, ROW2, "Lines"));
        assert!(get(ViewKind::Plan, ROW1, "View Selector"));
        assert!(!get(ViewKind::View3d, ROW2, "Lines"));
        assert!(get(ViewKind::View3d, ROW2, "Straight Wall"));
        assert!(!get(ViewKind::View3d, ROW1, "View Selector"));
        assert!(get(ViewKind::Elevation, ROW2, "Lines"));
        assert!(!get(ViewKind::Elevation, ROW2, "Straight Wall"));
        assert!(!get(ViewKind::Elevation, ROW1, "Down One Floor"));
        assert!(get(ViewKind::Layout, ROW2, "Page Setup"));
        assert!(!get(ViewKind::Layout, ROW2, "Straight Wall"));
        // Hidden lists hold the standard bar's own buttons that are off.
        let layout_row2 = cfg.view(ViewKind::Layout).unwrap().bar(ROW2).unwrap();
        assert!(layout_row2.hidden.contains(&"Straight Wall".to_string()));
        assert!(!layout_row2.hidden.contains(&"Page Setup".to_string()));
    }

    #[test]
    fn the_configuration_round_trips_through_json_and_a_file() {
        let mut cfg = ToolbarConfig::daniel_default();
        cfg.locked = true;
        {
            let b = cfg.view_mut(ViewKind::Plan).bar_mut(ROW2).unwrap();
            assert!(b.remove_key("Spline"));
            assert!(b.add("Road", Some(2)));
        }
        let id = cfg.view_mut(ViewKind::Layout).add_row("My Row");
        cfg.view_mut(ViewKind::Layout)
            .bar_mut(&id)
            .unwrap()
            .add("Save", None);
        let text = cfg.to_json();
        let back = ToolbarConfig::from_json(&text).unwrap();
        assert_eq!(back, cfg);
        let dir = std::env::temp_dir().join(format!("plan-studio-tb-{}", std::process::id()));
        let path = dir.join("toolbars.json");
        cfg.save_to(&path).unwrap();
        assert_eq!(ToolbarConfig::load_from(&path).unwrap(), cfg);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(ToolbarConfig::from_json("{ nope").is_err());
    }

    #[test]
    fn editing_keeps_the_hidden_list_and_separators_consistent() {
        let mut b = ToolbarConfig::default_view(ViewKind::Plan)
            .bar(ROW2)
            .unwrap()
            .clone();
        let n = b.items.len();
        assert!(b.remove_key("Select Objects"));
        // The separator after it is now leading and goes too.
        assert_ne!(b.items[0], SEPARATOR);
        assert!(b.hidden.contains(&"Select Objects".to_string()));
        assert!(b.items.len() < n - 1);
        assert!(b.add("Select Objects", Some(0)));
        assert!(!b.add("Select Objects", None), "no duplicates");
        assert!(!b.add("No Such Button", None));
        assert!(!b.hidden.contains(&"Select Objects".to_string()));
        // Reordering.
        let first = b.items[0].clone();
        assert!(b.move_item(0, 3));
        assert_eq!(b.items[3], first);
        assert!(!b.move_item(0, 999));
        // Separators never double up, lead or trail.
        b.add_separator(0);
        b.add_separator(b.items.len());
        assert_ne!(b.items[0], SEPARATOR);
        assert_ne!(b.items.last().unwrap(), SEPARATOR);
        assert!(b
            .items
            .windows(2)
            .all(|w| w[0] != SEPARATOR || w[1] != SEPARATOR));
    }

    #[test]
    fn rows_can_be_added_and_only_the_users_rows_deleted() {
        let mut s = ToolbarConfig::default_view(ViewKind::Plan);
        let id = s.add_row("Terrain");
        assert_eq!(id, "custom-1");
        assert_eq!(s.add_row("  "), "custom-2");
        assert!(s.bar(&id).unwrap().items.is_empty());
        assert!(s.bar_mut(&id).unwrap().add("Road", None));
        assert!(s.remove_row(&id));
        assert!(!s.remove_row(ROW1), "the standard bars stay");
        assert!(s.bar(&id).is_none());
    }

    #[test]
    fn normalize_repairs_a_file_from_another_version() {
        let mut cfg = ToolbarConfig::daniel_default();
        {
            let b = cfg.view_mut(ViewKind::Plan).bar_mut(ROW2).unwrap();
            b.items.push("A Button That Was Removed".into());
            b.items.push("Spline".into()); // a repeat
                                           // A button the file has never heard of: not listed, not hidden.
            b.items.retain(|k| k != "Paste Hold Position");
        }
        cfg.views.remove(&ViewKind::Layout);
        cfg.view_mut(ViewKind::View3d)
            .bars
            .retain(|b| b.id != VIEW_BAR);
        cfg.normalize();
        let b = cfg.view(ViewKind::Plan).unwrap().bar(ROW2).unwrap();
        assert!(!b.contains("A Button That Was Removed"));
        assert_eq!(b.keys().filter(|k| *k == "Spline").count(), 1);
        assert!(b.contains("Paste Hold Position"), "new buttons show up");
        // It lands next to its neighbour in the default order.
        let at = b
            .items
            .iter()
            .position(|k| k == "Paste Hold Position")
            .unwrap();
        assert_eq!(b.items[at - 1], "3D Solid");
        assert!(cfg.view(ViewKind::Layout).is_some());
        assert!(cfg.view(ViewKind::View3d).unwrap().bar(VIEW_BAR).is_some());
    }

    #[test]
    fn the_live_configuration_follows_the_active_view() {
        let mut cfg = ToolbarConfig::daniel_default();
        cfg.view_mut(ViewKind::View3d)
            .bar_mut(ROW1)
            .unwrap()
            .remove_key("Sun Angle");
        set_current_unsaved(cfg);
        set_active_view(ViewKind::Plan);
        assert_eq!(active_view(), ViewKind::Plan);
        let plan = with_current(|c| c.items_for(active_view(), ROW1))
            .unwrap()
            .1;
        assert!(plan.iter().any(|k| k == "Sun Angle"));
        set_active_view(ViewKind::View3d);
        let v3 = with_current(|c| c.items_for(active_view(), ROW1))
            .unwrap()
            .1;
        assert!(!v3.iter().any(|k| k == "Sun Angle"));
        set_active_view(ViewKind::Plan);
        set_current_unsaved(ToolbarConfig::default());
    }

    /// Runs one frame of `slots` as a row and returns the keys drawn, in order.
    fn drawn(slots: &mut [Slot]) -> Vec<String> {
        let flags = HashSet::new();
        let state = BarState {
            tool: crate::tools::ToolId::Select,
            flags: &flags,
            dock: None,
            floor: 0,
            floor_count: 2,
            view_name: "Floor Plan",
            views: &[],
            brightness: 1.0,
            undo_label: None,
            redo_label: None,
            hotkeys: None,
        };
        let ctx = egui::Context::default();
        DRAWN.with(|d| d.borrow_mut().clear());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = super::super::row(ui, slots, &state);
            });
        });
        DRAWN.with(|d| d.borrow().clone())
    }

    #[test]
    fn a_configured_bar_draws_only_its_buttons_in_its_order() {
        let mut cfg = ToolbarConfig::daniel_default();
        cfg.view_mut(ViewKind::Plan)
            .bar_mut(ROW2)
            .unwrap()
            .set_items(strs(&["Door", "|", "Select Objects"]));
        // A button from another bar and one no bar carries by default.
        cfg.view_mut(ViewKind::Plan)
            .bar_mut(ROW2)
            .unwrap()
            .add("Road", None);
        cfg.view_mut(ViewKind::Plan)
            .bar_mut(ROW2)
            .unwrap()
            .add("Save", Some(0));
        set_current_unsaved(cfg);
        set_active_view(ViewKind::Plan);
        assert_eq!(
            drawn(&mut row2_slots()),
            ["Save", "Door", "Select Objects", "Road"]
        );
        // Another view type has its own set.
        set_active_view(ViewKind::Elevation);
        let elev = drawn(&mut row2_slots());
        assert_eq!(elev[0], "Select Objects");
        assert!(!elev.iter().any(|k| k == "Straight Wall"));
        // A hidden bar draws nothing; row 1 is untouched.
        let mut cfg = current();
        cfg.view_mut(ViewKind::Elevation)
            .bar_mut(ROW2)
            .unwrap()
            .visible = false;
        set_current_unsaved(cfg);
        assert!(drawn(&mut row2_slots()).is_empty());
        assert!(drawn(&mut row1_slots()).len() > 15);
        set_active_view(ViewKind::Plan);
        set_current_unsaved(ToolbarConfig::default());
    }

    #[test]
    fn user_rows_draw_under_the_build_bar_only() {
        let mut cfg = ToolbarConfig::daniel_default();
        let id = cfg.view_mut(ViewKind::Plan).add_row("Mine");
        let b = cfg.view_mut(ViewKind::Plan).bar_mut(&id).unwrap();
        b.add("Zoom In", None);
        b.add("|", None);
        b.add("Plant", None);
        set_current_unsaved(cfg);
        set_active_view(ViewKind::Plan);
        let row2 = drawn(&mut row2_slots());
        assert_eq!(&row2[row2.len() - 2..], ["Zoom In", "Plant"]);
        assert!(!drawn(&mut row1_slots()).iter().any(|k| k == "Plant"));
        set_current_unsaved(ToolbarConfig::default());
    }

    #[test]
    fn chief_labels_map_by_table_and_by_name() {
        assert_eq!(map_chief_label("Door Tools"), Some("Door"));
        assert_eq!(
            map_chief_label("Straight Wall Tools"),
            Some("Straight Wall")
        );
        assert_eq!(map_chief_label("  save  "), Some("Save"));
        assert_eq!(map_chief_label("Select Objects"), Some("Select Objects"));
        assert_eq!(
            map_chief_label("Coordinate System Indicator - Floating"),
            None
        );
        // Every target is a real button.
        for (chief, key) in CHIEF_LABELS {
            assert!(entry(key).is_some(), "{chief} -> {key}");
        }
    }

    #[test]
    fn importing_daniels_toolbar_maps_at_least_80_percent_of_his_buttons() {
        let set = set();
        let buttons = plan_config::all_view_buttons(&set);
        let mapped = buttons
            .iter()
            .filter(|(_, label)| map_chief_label(label).is_some())
            .count();
        let ratio = mapped as f64 / buttons.len() as f64;
        eprintln!(
            "Daniel's Default Configuration: {mapped} of {} buttons map",
            buttons.len()
        );
        assert!(
            ratio >= 0.80,
            "{mapped} of {} mapped ({:.0}%); unmapped: {:?}",
            buttons.len(),
            ratio * 100.0,
            buttons
                .iter()
                .filter(|(_, l)| map_chief_label(l).is_none())
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_one_of_daniels_toolbar_sets_maps_mostly() {
        // The Default Configuration is the one he works in: 80%. The others
        // add contextual snap, selection and arc-mode toggles, the Space
        // Planning room builders and time logging, which have no
        // counterpart yet.
        for set in plan_config::load_daniel_config().toolbars {
            let buttons = plan_config::all_view_buttons(&set);
            let mapped = buttons
                .iter()
                .filter(|(_, l)| map_chief_label(l).is_some())
                .count();
            let need = if set.name == "Default Configuration" {
                80
            } else {
                60
            };
            assert!(
                mapped * 100 >= buttons.len() * need,
                "{}: {mapped} of {}; unmapped: {:?}",
                set.name,
                buttons.len(),
                buttons
                    .iter()
                    .filter(|(_, l)| map_chief_label(l).is_none())
                    .map(|(_, l)| l.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn the_plan_import_puts_buttons_on_their_bars_in_chiefs_order() {
        let report = import_chief(&set(), ViewKind::Plan);
        assert!(report.total > 60 && report.mapped * 10 >= report.total * 8);
        let row2 = report.set.bar(ROW2).unwrap();
        assert_eq!(row2.items[0], "Select Objects");
        let at = |k: &str| row2.items.iter().position(|x| x == k).unwrap();
        assert!(at("Straight Wall") < at("Railing and Deck"));
        assert!(at("Door") < at("Window"));
        assert!(report.set.bar(ROW1).unwrap().contains("Save"));
        assert!(report.set.bar(VIEW_BAR).unwrap().contains("Zoom In"));
        // Nothing lands twice.
        let mut all: Vec<&String> = report.set.bars.iter().flat_map(|b| b.keys()).collect();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
        // Applying it replaces the standard bars and keeps the user's rows.
        let mut cfg = ToolbarConfig::daniel_default();
        let id = cfg.view_mut(ViewKind::Plan).add_row("Mine");
        cfg.apply_import(&report);
        let plan = cfg.view(ViewKind::Plan).unwrap();
        assert!(plan.bar(&id).is_some());
        assert_eq!(
            plan.bar(ROW2).unwrap().items,
            report.set.bar(ROW2).unwrap().items
        );
        // The layout import carries the page tools.
        let layout = import_chief(&set(), ViewKind::Layout);
        assert!(layout.set.bar(ROW2).unwrap().contains("Page Table"));
        assert!(layout.summary().contains("Chief buttons"));
    }
}
